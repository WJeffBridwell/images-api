#!/usr/bin/env python3
from pymongo import MongoClient
import time
from datetime import datetime

def main():
    # MongoDB setup
    client = MongoClient('mongodb://localhost:27017/')
    db = client.media_metadata
    
    last_count = 0
    start_time = datetime.now()
    
    while True:
        current_count = db.content.count_documents({})
        current_time = datetime.now()
        elapsed = (current_time - start_time).total_seconds()
        
        if current_count > last_count:
            rate = current_count / elapsed if elapsed > 0 else 0
            print(f"\n{current_time.strftime('%H:%M:%S')} - Progress:")
            print(f"Total documents processed: {current_count:,}")
            print(f"Documents in last interval: {current_count - last_count:,}")
            print(f"Processing rate: {rate:.1f} docs/second")
            last_count = current_count
        
        time.sleep(5)  # Check every 5 seconds

if __name__ == "__main__":
    main()
