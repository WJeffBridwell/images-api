#!/usr/bin/env python3

import os
import sys
import subprocess
import hashlib
import json
import argparse
from datetime import datetime
from pathlib import Path
from pymongo import MongoClient
from typing import Dict, Any

def get_mdls_attributes(file_path: str) -> Dict[str, Any]:
    """Get macOS metadata attributes using mdls"""
    try:
        result = subprocess.run(['mdls', file_path], capture_output=True, text=True)
        if result.returncode != 0:
            print(f"Warning: mdls failed for {file_path}: {result.stderr}")
            return {}
        
        attributes = {}
        for line in result.stdout.splitlines():
            if "=" not in line:
                continue
            key, value = line.split("=", 1)
            key = key.strip()
            value = value.strip()
            # Remove quotes if present
            if value.startswith('"') and value.endswith('"'):
                value = value[1:-1]
            attributes[key] = value
        return attributes
    except Exception as e:
        print(f"Error getting mdls attributes for {file_path}: {e}")
        return {}

def get_xattr_attributes(file_path: str) -> Dict[str, Any]:
    """Get extended attributes using xattr"""
    try:
        # First get list of attributes
        result = subprocess.run(['xattr', file_path], capture_output=True, text=True)
        if result.returncode != 0:
            print(f"Warning: xattr failed for {file_path}: {result.stderr}")
            return {}
        
        attributes = {}
        # Process each attribute
        for attr in result.stdout.splitlines():
            attr = attr.strip()
            if not attr:
                continue
                
            # Get the value for this attribute as bytes
            value_result = subprocess.run(['xattr', '-px', attr, file_path], capture_output=True, text=True)
            if value_result.returncode == 0:
                # Store hex representation of binary data
                attributes[attr] = value_result.stdout.strip()
                
                # Also try to get human readable version if possible
                try:
                    text_result = subprocess.run(['xattr', '-p', attr, file_path], capture_output=True, text=True)
                    if text_result.returncode == 0:
                        attributes[f"{attr}_text"] = text_result.stdout.strip()
                except:
                    pass  # Ignore failures on text conversion
            
        return attributes
    except Exception as e:
        print(f"Error getting xattr attributes for {file_path}: {e}")
        return {}

def calculate_checksum(file_path: str) -> str:
    """Calculate SHA-256 checksum of file"""
    try:
        sha256_hash = hashlib.sha256()
        with open(file_path, "rb") as f:
            for byte_block in iter(lambda: f.read(4096), b""):
                sha256_hash.update(byte_block)
        return sha256_hash.hexdigest()
    except Exception as e:
        print(f"Error calculating checksum for {file_path}: {e}")
        return ""

def process_file(file_path: str, models_collection) -> None:
    """Process a single file and add it to MongoDB"""
    try:
        path_obj = Path(file_path)
        stats = path_obj.stat()
        
        # Get all metadata
        mdls_data = get_mdls_attributes(file_path)
        xattr_data = get_xattr_attributes(file_path)
        checksum = calculate_checksum(file_path)
        
        # Create document
        document = {
            "path": str(path_obj),
            "filename": path_obj.name,
            "created_at": datetime.now(),
            "updated_at": datetime.now(),
            "checksum": checksum,
            "base_attributes": {
                "size": stats.st_size,
                "creation_time": datetime.fromtimestamp(stats.st_ctime),
                "modification_time": datetime.fromtimestamp(stats.st_mtime),
                "access_time": datetime.fromtimestamp(stats.st_atime)
            },
            "macos_attributes": {
                "mdls": mdls_data,
                "xattr": xattr_data
            }
        }
        
        # Insert into MongoDB
        result = models_collection.insert_one(document)
        print(f"Processed: {file_path} -> MongoDB ID: {result.inserted_id}")
        
    except Exception as e:
        print(f"Error processing file {file_path}: {e}")

def main():
    # Parse arguments
    parser = argparse.ArgumentParser(description='Load models into MongoDB')
    parser.add_argument('--truncate', action='store_true', help='Truncate models collection before loading')
    args = parser.parse_args()

    # MongoDB connection
    client = MongoClient('mongodb://localhost:27017/')
    db = client.media
    models_collection = db.models
    
    # Optionally truncate the collection
    if args.truncate:
        print("Truncating models collection...")
        models_collection.delete_many({})
    else:
        print("Skipping truncation, will append to existing models collection")
    
    # Base directory for models
    models_dir = "/Volumes/VideosNew/Models"
    
    # Count total files for progress tracking
    total_files = sum(len(files) for _, _, files in os.walk(models_dir))
    processed_files = 0
    
    # Walk through all files
    for root, dirs, files in os.walk(models_dir):
        for file in files:
            # Skip hidden files
            if file.startswith('.'):
                continue
                
            file_path = os.path.join(root, file)
            processed_files += 1
            print(f"Processing [{processed_files}/{total_files}]: {file_path}")
            process_file(file_path, models_collection)
            
    print(f"\nCompleted processing {processed_files} files")

if __name__ == "__main__":
    main()
