#!/usr/bin/env python3
import os
import mimetypes
from collections import defaultdict
from datetime import datetime
import json
import sys
from typing import Dict, List, Tuple

PATHS = [
    "/Volumes/VideosNew",
    "/Users/jeffbridwell/VideosAa-Abella",
    "/Volumes/VideosAbella-Alexa",
    "/VideosAlexa-Ame",
    "/Volumes/VideosAme-Aria",
    "/Volumes/VideosApr-Aria",
    "/Volumes/VideosAria-Bianca",
    "/Volumes/VideosBianka-Chan",
    "/Volumes/VVidoesChan-Coco",
    "/Volumes/VideosCoco-Eliza",
    "/Volumes/VideosEliza-Erica",
    "/Volumes/VideosErica-Haley",
    "/Volumes/VideosHaley-Hime",
    "/Volumes/VideosHime-Jeff",
    "/Volumes/VideosJeff-Kata",
    "/Volumes/VideosNew/VideosKata-Kenn",
    "/Volumes/PhotosNew/VideosKenn-Kenz",
    "/Users/jeffbridwell/VideoKenzie-Kev",
    "/Volumes/VideosJulia-Kg",
    "/Volumes/VideosKey-Lea",
    "/Volumes/VideosLeb-Luci",
    "/Volumes/VideosLucj-Maria",
    "/Volumes/VideosMaria-Mega",
    "/Volumes/VideosMega-Mia",
    "/Volumes/VidoesMia-Nat",
    "/Volumes/VideosNew/VideosNat-Nia",
    "/Volumes/VideosNia-Rilex",
    "/Users/jeffbridwell/VideosRiley",
    "/Volumes/VideosRIlez-Ta",
    "/Volumes/VideosTb-Uma",
    "/Volumes/Uma-Zaa",
    "/Volumes/VideosNew/VideosZaa-Zz"
]

class ContentAnalyzer:
    def __init__(self):
        self.stats = defaultdict(lambda: {
            'total_files': 0,
            'total_size': 0,
            'by_type': defaultdict(int),
            'by_extension': defaultdict(int),
            'accessible': True,
            'error': None
        })
        
    def analyze_path(self, root_path: str) -> None:
        """Analyze a single root path"""
        if not os.path.exists(root_path):
            self.stats[root_path]['accessible'] = False
            self.stats[root_path]['error'] = 'Path does not exist'
            print(f"❌ {root_path}: Not accessible", file=sys.stderr)
            return
            
        try:
            for dirpath, _, filenames in os.walk(root_path):
                print(f"Scanning: {dirpath}")
                
                for filename in filenames:
                    if filename.startswith('.'):
                        continue
                        
                    full_path = os.path.join(dirpath, filename)
                    try:
                        # Get file size
                        file_size = os.path.getsize(full_path)
                        
                        # Get content type
                        content_type, _ = mimetypes.guess_type(full_path)
                        if not content_type:
                            content_type = 'unknown'
                        
                        # Get extension
                        ext = os.path.splitext(filename)[1].lower()
                        
                        # Update stats
                        self.stats[root_path]['total_files'] += 1
                        self.stats[root_path]['total_size'] += file_size
                        self.stats[root_path]['by_type'][content_type] += 1
                        self.stats[root_path]['by_extension'][ext] += 1
                        
                    except Exception as e:
                        print(f"Error processing {full_path}: {e}", file=sys.stderr)
                        
        except Exception as e:
            self.stats[root_path]['accessible'] = False
            self.stats[root_path]['error'] = str(e)
            print(f"❌ {root_path}: {e}", file=sys.stderr)
    
    def analyze_all(self) -> None:
        """Analyze all configured paths"""
        for path in PATHS:
            print(f"\nAnalyzing {path}...", file=sys.stderr)
            self.analyze_path(path)
            
    def format_size(self, size: int) -> str:
        """Format size in bytes to human readable format"""
        for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
            if size < 1024:
                return f"{size:.2f} {unit}"
            size /= 1024
        return f"{size:.2f} PB"
    
    def print_summary(self) -> None:
        """Print analysis summary"""
        total_files = 0
        total_size = 0
        total_by_type = defaultdict(int)
        total_by_extension = defaultdict(int)
        inaccessible_paths = []
        
        # Aggregate totals
        for path, stats in self.stats.items():
            if stats['accessible']:
                total_files += stats['total_files']
                total_size += stats['total_size']
                for content_type, count in stats['by_type'].items():
                    total_by_type[content_type] += count
                for ext, count in stats['by_extension'].items():
                    total_by_extension[ext] += count
            else:
                inaccessible_paths.append((path, stats['error']))
        
        # Print overall summary
        print("\n" + "=" * 80)
        print("CONTENT ANALYSIS SUMMARY")
        print("=" * 80)
        
        print(f"\nTotal Files: {total_files:,}")
        print(f"Total Size: {self.format_size(total_size)}")
        
        print("\nBreakdown by Content Type:")
        for content_type, count in sorted(total_by_type.items(), key=lambda x: x[1], reverse=True):
            print(f"  {content_type}: {count:,}")
            
        print("\nBreakdown by Extension:")
        for ext, count in sorted(total_by_extension.items(), key=lambda x: x[1], reverse=True):
            print(f"  {ext}: {count:,}")
        
        if inaccessible_paths:
            print("\nInaccessible Paths:")
            for path, error in inaccessible_paths:
                print(f"  ❌ {path}: {error}")
        
        # Print per-path summary
        print("\nPer-Path Summary:")
        print("-" * 80)
        for path, stats in sorted(self.stats.items()):
            if stats['accessible']:
                print(f"\n{path}:")
                print(f"  Files: {stats['total_files']:,}")
                print(f"  Size: {self.format_size(stats['total_size'])}")
                
                # Show top 5 content types
                if stats['by_type']:
                    print("  Top Content Types:")
                    for ctype, count in sorted(stats['by_type'].items(), key=lambda x: x[1], reverse=True)[:5]:
                        print(f"    {ctype}: {count:,}")
        
        # Save detailed stats to JSON
        with open('content_analysis.json', 'w') as f:
            json.dump(self.stats, f, indent=2, default=str)
        print("\nDetailed statistics saved to content_analysis.json")

def main():
    analyzer = ContentAnalyzer()
    analyzer.analyze_all()
    analyzer.print_summary()

if __name__ == "__main__":
    main()
