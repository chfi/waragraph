#!/bin/bash

# Check if two arguments are given
if [ "$#" -ne 2 ]; then
    echo "Usage: $0 path1 path2"
    exit 1
fi

# Resolve the absolute paths of the input arguments
abs_path1=$(realpath "$1")
abs_path2=$(realpath "$2")

# Get the directory of the script itself
script_dir=$(dirname "$0")

cd $script_dir

# Define the executable path relative to the script location
# Replace 'relative/path/to/executable' with the actual relative path
executable="../target/release/waragraph-server"

# Run the executable with the absolute paths
"$executable" "$abs_path1" "$abs_path2"
