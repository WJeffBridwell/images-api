#!/usr/bin/env python3
import os
import sys
import json
import time
import hashlib
import mimetypes
from datetime import datetime
import multiprocessing as mp
from functools import partial
from pymongo import MongoClient
import subprocess
import argparse
import signal

# Force stdout to be unbuffered
if hasattr(sys.stdout, 'reconfigure'):
    sys.stdout.reconfigure(line_buffering=True)
else:
    sys.stdout = os.fdopen(sys.stdout.fileno(), 'w', buffering=1)

def signal_handler(signum, frame):
    print("\nReceived signal to terminate. Cleaning up...", file=sys.stderr, flush=True)
    sys.exit(0)

signal.signal(signal.SIGTERM, signal_handler)
signal.signal(signal.SIGINT, signal_handler)

def calculate_checksum(file_path):
    sha256_hash = hashlib.sha256()
    with open(file_path, "rb") as f:
        for byte_block in iter(lambda: f.read(4096), b""):
            sha256_hash.update(byte_block)
    return sha256_hash.hexdigest()

def get_base_attributes(file_path):
    stat = os.stat(file_path)
    return {
        "size": stat.st_size,
        "created": stat.st_ctime,
        "modified": stat.st_mtime,
        "accessed": stat.st_atime
    }

def get_mdls_attributes(file_path):
    start_time = time.time()
    try:
        result = subprocess.run(['mdls', file_path], capture_output=True, text=True)
        if result.returncode == 0:
            lines = result.stdout.split('\n')
            attrs = {}
            for line in lines:
                if '=' in line:
                    key, value = line.split('=', 1)
                    attrs[key.strip()] = value.strip()
            return attrs, time.time() - start_time
        return {}, time.time() - start_time
    except Exception:
        return {}, time.time() - start_time

def get_xattr_attributes(file_path):
    start_time = time.time()
    try:
        result = subprocess.run(['xattr', '-l', file_path], capture_output=True, text=True)
        if result.returncode == 0:
            return {"xattr_data": result.stdout}, time.time() - start_time
        return {}, time.time() - start_time
    except Exception:
        return {}, time.time() - start_time

def get_image_metadata(file_path):
    start_time = time.time()
    try:
        result = subprocess.run(['identify', '-verbose', file_path], capture_output=True, text=True)
        if result.returncode == 0:
            return {"identify_data": result.stdout}, time.time() - start_time
        return {}, time.time() - start_time
    except Exception:
        return {}, time.time() - start_time

def get_video_metadata(file_path):
    start_time = time.time()
    try:
        result = subprocess.run(['ffprobe', '-v', 'quiet', '-print_format', 'json', '-show_format', '-show_streams', file_path], capture_output=True, text=True)
        if result.returncode == 0:
            return json.loads(result.stdout), time.time() - start_time
        return {}, time.time() - start_time
    except Exception:
        return {}, time.time() - start_time

def process_file(file_path: str, mongo_uri: str):
    """Process a single file and return metadata dict"""
    start_time = time.time()
    try:
        # Skip hidden files
        if os.path.basename(file_path).startswith('.'):
            print(f"Skipping hidden file: {file_path}", file=sys.stderr, flush=True)
            return (None, 0, {})
            
        # Determine content type
        content_type = mimetypes.guess_type(file_path)[0]
        if not content_type:
            print(f"Could not determine content type for: {file_path}", file=sys.stderr, flush=True)
            return (None, 0, {})

        # Prepare base document
        doc = {}
        timings = {}
        
        try:
            # Get base attributes and checksum
            base_start = time.time()
            doc.update({
                "file_path": file_path,
                "content_type": content_type,
                "base_attributes": get_base_attributes(file_path),
            })
            timings['base'] = time.time() - base_start

            # Get extended attributes
            mdls_data, mdls_time = get_mdls_attributes(file_path)
            xattr_data, xattr_time = get_xattr_attributes(file_path)
            doc["extended_attributes"] = {
                "mdls": mdls_data,
                "xattr": xattr_data
            }
            timings['mdls'] = mdls_time
            timings['xattr'] = xattr_time

        except Exception as e:
            print(f"Error getting base metadata for {file_path}: {e}", file=sys.stderr, flush=True)

        # Add media-specific metadata
        if content_type.startswith('video/'):
            try:
                video_data, video_time = get_video_metadata(file_path)
                doc["video_metadata"] = video_data
                timings['video'] = video_time
            except Exception as e:
                print(f"Error getting video metadata for {file_path}: {e}", file=sys.stderr, flush=True)
                timings['video'] = 0
        elif content_type.startswith('image/'):
            try:
                image_data, image_time = get_image_metadata(file_path)
                doc["image_metadata"] = image_data
                timings['image'] = image_time
            except Exception as e:
                print(f"Error getting image metadata for {file_path}: {e}", file=sys.stderr, flush=True)
                timings['image'] = 0

        duration = time.time() - start_time
        return (doc, duration, timings)
    except Exception as e:
        print(f"❌ Failed to process {file_path}: {str(e)}", file=sys.stderr, flush=True)
        return (None, time.time() - start_time, {})

def worker_init():
    """Initialize worker process"""
    mimetypes.init()

def gather_files(directory):
    print(f"Gathering file list from {directory}...", flush=True)
    
    all_files = []
    for root, _, files in os.walk(directory):
        for file in files:
            if not file.startswith('.'):  # Skip hidden files
                all_files.append(os.path.join(root, file))
    
    print(f"Found {len(all_files)} files in {directory}", flush=True)
    return all_files

def main(args):
    root_dir = os.path.abspath(args.root_directory)
    if not os.path.isdir(root_dir):
        print(f"Error: {root_dir} is not a directory", file=sys.stderr, flush=True)
        sys.exit(1)

    # MongoDB setup
    mongo_uri = args.mongo_uri if args.mongo_uri else "mongodb://localhost:27017"
    client = MongoClient(mongo_uri)
    db = client.media

    # Optionally truncate content collection
    if args.truncate:
        print("Truncating content collection...", file=sys.stderr, flush=True)
        db.content.delete_many({})
    else:
        print("Skipping truncation, will append to existing content collection", file=sys.stderr, flush=True)

    BATCH_SIZE = 20
    
    print("\nGathering file list...", file=sys.stderr, flush=True)
    gather_start = time.time()
    
    all_files = gather_files(root_dir)
    gather_duration = time.time() - gather_start
    total_files = len(all_files)
    
    print(f"\nFound {total_files:,} files in {gather_duration:.1f} seconds", file=sys.stderr, flush=True)
    
    process_start = time.time()
    processed_count = 0
    total_processing_time = 0  # Track total time spent processing files
    
    # Use fewer workers to reduce contention
    worker_count = min(mp.cpu_count(), 8)  # Cap at 8 workers
    
    print(f"\nStarting processing with {worker_count} workers", file=sys.stderr, flush=True)
    
    with mp.Pool(worker_count, initializer=worker_init) as pool:
        try:
            process_func = partial(process_file, mongo_uri=args.mongo_uri)
            
            results_batch = []
            success_count = 0
            error_count = 0
            
            for result, duration, timings in pool.imap_unordered(process_func, all_files):
                try:
                    processed_count += 1
                    total_processing_time += duration
                    
                    # Calculate throughput metrics
                    elapsed_time = time.time() - process_start
                    overall_throughput = processed_count / elapsed_time if elapsed_time > 0 else 0
                    avg_processing_time = total_processing_time / processed_count if processed_count > 0 else 0
                    
                    # Get file size
                    file_size = os.path.getsize(all_files[processed_count-1])
                    file_size_mb = file_size / (1024 * 1024)
                    
                    # Format timing details
                    timing_details = " - ".join([
                        f"{op}: {t:.2f}s" for op, t in timings.items()
                        if t > 0  # Only show operations that were actually performed
                    ])
                    
                    # Progress output with throughput metrics, file size, and timings
                    print(f"Progress: {processed_count}/{total_files} ({processed_count/total_files*100:.1f}%) - "
                          f"File size: {file_size_mb:.2f}MB - "
                          f"File took: {duration:.2f}s ({timing_details}) - "
                          f"Avg processing: {avg_processing_time:.2f}s/doc - "
                          f"Overall rate: {overall_throughput:.1f} docs/sec total", 
                          flush=True)
                    
                    if result:
                        results_batch.append(result)
                        success_count += 1
                        
                        # Batch insert when we reach BATCH_SIZE
                        if len(results_batch) >= BATCH_SIZE:
                            try:
                                insert_start = time.time()
                                db.content.insert_many(results_batch)
                                insert_duration = time.time() - insert_start
                                batch_rate = len(results_batch) / insert_duration
                                
                                # Calculate total batch size
                                batch_size = sum(os.path.getsize(doc['file_path']) for doc in results_batch)
                                batch_size_mb = batch_size / (1024 * 1024)
                                
                                print(f"Committed {len(results_batch)} new documents ({batch_size_mb:.2f}MB) in {insert_duration:.2f}s "
                                      f"({batch_rate:.1f} docs/sec) - "
                                      f"completed {processed_count} of {total_files} - "
                                      f"% complete {(processed_count/total_files)*100:.2f}", 
                                      flush=True)
                                results_batch = []
                            except Exception as e:
                                print(f"Failed to insert batch: {e}", file=sys.stderr, flush=True)
                                error_count += len(results_batch)
                                success_count -= len(results_batch)
                                results_batch = []
                    else:
                        error_count += 1
                except BrokenPipeError:
                    print("Output pipe broken. Continuing silently.", file=sys.stderr, flush=True)
                except Exception as e:
                    print(f"Error processing result: {e}", file=sys.stderr, flush=True)
                    error_count += 1
                    
        except KeyboardInterrupt:
            print("\nReceived keyboard interrupt. Cleaning up...", file=sys.stderr, flush=True)
            pool.terminate()
            pool.join()
            sys.exit(1)
        except Exception as e:
            print(f"Error in main processing loop: {e}", file=sys.stderr, flush=True)
            pool.terminate()
            pool.join()
            sys.exit(1)
            
    # Insert any remaining results
    if results_batch:
        try:
            insert_start = time.time()
            db.content.insert_many(results_batch)
            insert_duration = time.time() - insert_start
            batch_rate = len(results_batch) / insert_duration
            
            # Calculate total batch size
            batch_size = sum(os.path.getsize(doc['file_path']) for doc in results_batch)
            batch_size_mb = batch_size / (1024 * 1024)
            
            print(f"Committed final {len(results_batch)} new documents ({batch_size_mb:.2f}MB) in {insert_duration:.2f}s "
                  f"({batch_rate:.1f} docs/sec) - "
                  f"completed {processed_count} of {total_files} - "
                  f"% complete {(processed_count/total_files)*100:.2f}", 
                  flush=True)
        except Exception as e:
            print(f"Failed to insert final batch: {e}", file=sys.stderr, flush=True)
            error_count += len(results_batch)
            success_count -= len(results_batch)
            
    end_time = time.time()
    total_duration = end_time - process_start
    
    # Final stats to stderr
    print("\nFinal Results:", file=sys.stderr, flush=True)
    print(f"Total files processed: {total_files:,}", file=sys.stderr, flush=True)
    print(f"Successful: {success_count:,}", file=sys.stderr, flush=True)
    print(f"Failed: {error_count:,}", file=sys.stderr, flush=True)
    print(f"Total time: {total_duration/60:.1f} minutes", file=sys.stderr, flush=True)
    print(f"Final rate: {total_files/total_duration*60:.1f} files/minute", file=sys.stderr, flush=True)
    print(f"Documents in DB: {db.content.count_documents({}):,}", file=sys.stderr, flush=True)

if __name__ == "__main__":
    parser = argparse.ArgumentParser(description='Load content into MongoDB')
    parser.add_argument('root_directory', help='Root directory to scan for content')
    parser.add_argument('--truncate', action='store_true', help='Truncate content collection before loading')
    parser.add_argument('--mongo-uri', dest='mongo_uri', help='MongoDB URI (default: mongodb://localhost:27017)')
    args = parser.parse_args()
    main(args)
