#!/bin/bash

# Array of directories to check
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

echo "Checking directory accessibility..."
echo "================================="

accessible=0
not_accessible=0

for dir in "${directories[@]}"; do
    if [ -d "$dir" ]; then
        echo "✅ Accessible: $dir"
        ((accessible++))
    else
        echo "❌ Not accessible: $dir"
        ((not_accessible++))
    fi
done

echo "================================="
echo "Summary:"
echo "Accessible directories: $accessible"
echo "Inaccessible directories: $not_accessible"
echo "Total directories checked: $((accessible + not_accessible))"
