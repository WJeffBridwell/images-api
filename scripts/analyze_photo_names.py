#!/usr/bin/env python3

import os
import re
import sys
import shutil
from pathlib import Path
from datetime import datetime

class Logger:
    def __init__(self, log_file):
        self.terminal = sys.stdout
        self.log = open(log_file, 'w', encoding='utf-8')
        
    def write(self, message):
        self.terminal.write(message)
        self.log.write(message)
        self.log.flush()
        
    def flush(self):
        self.terminal.flush()
        self.log.flush()

def should_rename_file(filename, parent_folder):
    """
    Check if the filename should be renamed based on parent folder metadata.
    Returns True if the parent folder name contains information not present in the filename.
    """
    # Convert both to lowercase for case-insensitive comparison
    filename_lower = filename.lower()
    parent_lower = parent_folder.lower()
    
    # Split parent folder name into potential metadata parts
    parent_parts = set(re.split(r'[-_\s]+', parent_lower))
    filename_parts = set(re.split(r'[-_\s]+', os.path.splitext(filename_lower)[0]))
    
    # Check if there are meaningful parts in parent folder not present in filename
    meaningful_parent_parts = {part for part in parent_parts if len(part) > 2}  # Skip very short parts
    meaningful_filename_parts = {part for part in filename_parts if len(part) > 2}
    
    # Return True if there are meaningful parts in parent folder not present in filename
    return bool(meaningful_parent_parts - meaningful_filename_parts)

def generate_new_name(filename, parent_folder):
    """Generate a new filename that includes parent folder metadata."""
    name, ext = os.path.splitext(filename)
    return f"{parent_folder}_{name}{ext}"

def rename_files(dirpath, files_to_rename, parent_folder, dry_run=True):
    """
    Perform the actual renaming of files with safety checks.
    Returns a tuple of (success_count, error_count, errors)
    """
    success_count = 0
    error_count = 0
    errors = []
    
    for filename in files_to_rename:
        old_path = os.path.join(dirpath, filename)
        new_name = generate_new_name(filename, parent_folder)
        new_path = os.path.join(dirpath, new_name)
        
        # Safety checks
        if not os.path.exists(old_path):
            errors.append(f"Source file does not exist: {old_path}")
            error_count += 1
            continue
            
        if os.path.exists(new_path):
            errors.append(f"Target file already exists: {new_path}")
            error_count += 1
            continue
        
        try:
            if not dry_run:
                os.rename(old_path, new_path)
            success_count += 1
            print(f"{'[DRY RUN] Would rename' if dry_run else 'Renamed'}: {filename} -> {new_name}")
        except Exception as e:
            errors.append(f"Error renaming {filename}: {str(e)}")
            error_count += 1
    
    return success_count, error_count, errors

def analyze_directory(root_path, perform_rename=False, dry_run=True):
    """
    Recursively analyze directories and rename files if requested.
    """
    # Common image extensions
    image_extensions = {'.jpg', '.jpeg', '.png', '.gif', '.bmp', '.tiff', '.webp'}
    
    total_dirs = 0
    total_images = 0
    total_rename_suggestions = 0
    total_renames_success = 0
    total_renames_failed = 0
    unchanged_files = []
    all_errors = []
    
    print(f"\nStarting analysis at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"Root directory: {root_path}")
    print(f"Mode: {'Dry run' if dry_run else 'Live rename' if perform_rename else 'Analysis only'}")
    print("-" * 80)
    
    # Process files in the root directory first
    root_parent_folder = os.path.basename(root_path)
    root_files = [f for f in os.listdir(root_path) if os.path.isfile(os.path.join(root_path, f))]
    root_image_files = [f for f in root_files if os.path.splitext(f)[1].lower() in image_extensions]
    
    if root_image_files:
        total_dirs += 1
        total_images += len(root_image_files)
        files_to_rename = [f for f in root_image_files if should_rename_file(f, root_parent_folder)]
        unchanged = set(root_image_files) - set(files_to_rename)
        
        if unchanged:
            for filename in unchanged:
                unchanged_files.append((root_path, filename, root_parent_folder))
        
        if files_to_rename:
            total_rename_suggestions += len(files_to_rename)
            print(f"\nDirectory: {root_path}")
            print(f"Parent folder: {root_parent_folder}")
            print(f"Total images in folder: {len(root_image_files)}")
            print("Files to process:")
            
            if perform_rename:
                success, failed, errors = rename_files(root_path, files_to_rename, root_parent_folder, dry_run)
                total_renames_success += success
                total_renames_failed += failed
                all_errors.extend(errors)
            else:
                for filename in files_to_rename:
                    new_name = generate_new_name(filename, root_parent_folder)
                    print(f"  {filename} -> {new_name}")
    
    # Process subdirectories
    for dirpath, dirnames, filenames in os.walk(root_path):
        if dirpath == root_path:
            continue
            
        parent_folder = os.path.basename(dirpath)
        image_files = [f for f in filenames if os.path.splitext(f)[1].lower() in image_extensions]
        
        if not image_files:
            continue
            
        total_dirs += 1
        total_images += len(image_files)
        
        files_to_rename = [f for f in image_files if should_rename_file(f, parent_folder)]
        unchanged = set(image_files) - set(files_to_rename)
        
        if unchanged:
            for filename in unchanged:
                unchanged_files.append((dirpath, filename, parent_folder))
        
        if files_to_rename:
            total_rename_suggestions += len(files_to_rename)
            print(f"\nDirectory: {dirpath}")
            print(f"Parent folder: {parent_folder}")
            print(f"Total images in folder: {len(image_files)}")
            print("Files to process:")
            
            if perform_rename:
                success, failed, errors = rename_files(dirpath, files_to_rename, parent_folder, dry_run)
                total_renames_success += success
                total_renames_failed += failed
                all_errors.extend(errors)
            else:
                for filename in files_to_rename:
                    new_name = generate_new_name(filename, parent_folder)
                    print(f"  {filename} -> {new_name}")
    
    print("\n" + "=" * 80)
    print("Analysis Summary:")
    print(f"Total directories processed: {total_dirs}")
    print(f"Total images found: {total_images}")
    print(f"Total rename suggestions: {total_rename_suggestions}")
    print(f"Files not requiring rename: {len(unchanged_files)}")
    
    if perform_rename:
        print(f"\nRename Operations:")
        print(f"Successfully processed: {total_renames_success}")
        print(f"Failed operations: {total_renames_failed}")
        
        if all_errors:
            print("\nErrors encountered:")
            for error in all_errors:
                print(f"- {error}")
    
    print("=" * 80)
    
    if unchanged_files:
        print("\nFiles that already contain sufficient metadata:")
        print("-" * 80)
        for dirpath, filename, parent_folder in unchanged_files:
            print(f"Directory: {dirpath}")
            print(f"File: {filename}")
            print("-" * 40)
    
    print(f"\nAnalysis completed at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")

def main():
    if len(sys.argv) < 2:
        print("Usage: python3 analyze_photo_names.py <directory> [--rename] [--live]")
        print("Options:")
        print("  --rename  Perform rename operations (dry run by default)")
        print("  --live    Perform actual renames instead of dry run")
        return
        
    photo_dir = sys.argv[1]
    perform_rename = "--rename" in sys.argv
    live_mode = "--live" in sys.argv
    
    log_file = "photo_rename_analysis.log"
    
    # Set up logging
    sys.stdout = Logger(log_file)
    
    if not os.path.exists(photo_dir):
        print(f"Error: Directory '{photo_dir}' does not exist")
        return
    
    print(f"Analysis started at: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    print(f"Analyzing directory structure in: {photo_dir}")
    print(f"Log file: {log_file}")
    
    analyze_directory(photo_dir, perform_rename=perform_rename, dry_run=not live_mode)

if __name__ == "__main__":
    main()
