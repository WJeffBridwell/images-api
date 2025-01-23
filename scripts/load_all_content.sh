#!/bin/bash

# Array of directories to process
directories=(
    "/Users/jeffbridwell/VideosAa-Abella"
    "/Volumes/VideosAbella-Alexa"
    "/Volumes/VideosAlexa-Ame"
    "/Volumes/VideosAme-Aria"
    "/Volumes/VideosAria-Bianca"
    "/Volumes/VideosBianka-Chan"
    "/Volumes/VideosChan-Coco"
    "/Volumes/VideosCoco-Eliza"
    "/Volumes/VideosEliza-Erica"
    "/Volumes/VideosErica-Haley"
    "/Volumes/VideosHaley-Hime"
    "/Volumes/VideosHime-Jeff"
    "/Volumes/VideosJeff-Kata"
    "/Volumes/VideosNew/New/VideosKata-Kenn"
    "/Volumes/PhotosNew/VideosKenn-Kenz"
    "/Users/jeffbridwell/VideosKenzie-Kev"
    "/Volumes/VideosKey-Lea"
    "/Volumes/VideosLeb-Luci"
    "/Volumes/VideosLucj-Maria"
    "/Volumes/VideosMaria-Mega"
    "/Volumes/VideosMega-Mia"
    "/Volumes/VideosMia-Nat"
    "/Volumes/VideosNew/VideosNat-Nia"
    "/Volumes/VideosNia-Rilex"
    "/Users/jeffbridwell/VideosRiley"
    "/Volumes/VideosRilez-Ta"
    "/Volumes/VideosTb-Uma"
    "/Volumes/VideosUma-Zaa"
    "/Volumes/VideosNew/VideosZaa-Zz"
)

SCRIPT_DIR="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
PARENT_DIR="$( cd "$SCRIPT_DIR/.." && pwd )"

# Ensure logs directory exists
mkdir -p "$PARENT_DIR/logs"

# Redirect all output to log file
exec 1> >(tee -a "$PARENT_DIR/logs/load_all_content.log")
exec 2>&1

echo "Starting content loading process at $(date)"

# Process first directory with truncate flag
first_dir="${directories[0]}"
echo "Starting load for first directory with truncate: $first_dir"
python3 "$SCRIPT_DIR/load_content.py" "$first_dir" --truncate
echo "Completed load for first directory with truncate: $first_dir"

# Process remaining directories without truncate
for dir in "${directories[@]:1}"; do
    if [ -d "$dir" ]; then
        echo "Starting load for directory: $dir"
        python3 "$SCRIPT_DIR/load_content.py" "$dir"
        echo "Completed load for directory: $dir"
    else
        echo "Directory not found: $dir"
    fi
done

echo "Content loading process complete at $(date)"
