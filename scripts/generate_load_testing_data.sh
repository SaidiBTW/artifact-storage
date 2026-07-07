#!/bin/bash

OUTPUT_DIR="test_files"

TARGET_PATH="$(pwd)/$OUTPUT_DIR"

echo "DATA GENERATION SCRIPT"
echo "This script uses the dd command which can be dangerous if configured incorrectly"
echo "Current directory is $TARGET_PATH"
echo "----------------------------------------------------------------------------------"


read -p "Are you sure to proceed? (y/n) " -n 1 -r
echo ""

if [[ ! $REPLY =~ ^[Yy]$ ]]
then 
  echo "Process cancelled. No files created"
  exit 1
fi


echo "Starting data file generation in ./$OUTPUT_DIR... using the \`dd\` linux util"
echo "This generation will occupy ~8GB of storage on your computer"
echo "Please run the cleanup function at the end or delete the $OUTPUT_DIR directory to reclaim your space"
echo "----------------------------------------------------------------------------------"

mkdir -p "$OUTPUT_DIR"

# Function to create a file of a specfic size
generate_clean_file(){
  local filename=$1
  local block_size=$2
  local count=$3

  echo "-> Generating ${filename}..."
  
  dd if=/dev/urandom of="$OUTPUT_DIR/$filename" bs="$block_size" count="$count" 2>/dev/null 
}

generate_corrupt_file(){
  local source_file=$1
  local corrupt_file=$2
  local total_bytes=$3

  echo "-> Creating corrupted copy: ${corrupt_file}..."
  cp "$OUTPUT_DIR/$source_file" "${OUTPUT_DIR}/$corrupt_file"

  #Find the middle of the file
  local seek_position=$(( total_bytes / 2 ))

  #Overwrite one file to create a corrupt copy
  dd if=/dev/urandom of="$OUTPUT_DIR/$corrupt_file" bs=1 seek="$seek_position" count=1 conv=notrunc 2>/dev/null
}

#Generate Clean Files
generate_clean_file "5KB_clean.bin" 1k 5
generate_clean_file "10KB_clean.bin" 1k 10
generate_clean_file "20KB_clean.bin" 1k 20
generate_clean_file "30KB_clean.bin" 1k 30
generate_clean_file "40KB_clean.bin" 1k 40
generate_clean_file "50KB_clean.bin" 1k 50
generate_clean_file "60KB_clean.bin" 1k 60
generate_clean_file "70KB_clean.bin" 1k 70
generate_clean_file "80KB_clean.bin" 1k 80
generate_clean_file "90KB_clean.bin" 1k 90
generate_clean_file "100KB_clean.bin" 1k 100
generate_clean_file "200KB_clean.bin" 1k 200
generate_clean_file "300KB_clean.bin" 1k 300
generate_clean_file "400KB_clean.bin" 1k 400
generate_clean_file "500KB_clean.bin" 1k 500
generate_clean_file "600KB_clean.bin" 1k 600
generate_clean_file "700KB_clean.bin" 1k 700
generate_clean_file "800KB_clean.bin" 1k 800
generate_clean_file "900KB_clean.bin" 1k 900
generate_clean_file "1MB_clean.bin" 1m 1
generate_clean_file "2MB_clean.bin" 1m 2
generate_clean_file "3MB_clean.bin" 1m 3
generate_clean_file "4MB_clean.bin" 1m 4
generate_clean_file "5MB_clean.bin" 1m 5
generate_clean_file "6MB_clean.bin" 1m 6
generate_clean_file "7MB_clean.bin" 1m 7
generate_clean_file "8MB_clean.bin" 1m 8
generate_clean_file "9MB_clean.bin" 1m 9
generate_clean_file "10MB_clean.bin" 1m 10


# Generate Corrupted Files
generate_corrupt_file "5KB_clean.bin" "5KB_corrupt.bin" 5120
generate_corrupt_file "5MB_clean.bin" "5MB_corrupt.bin" 5242880

echo "-----------------------------------"
echo "All files genereated in the '$OUTPUT_DIR' folder."