# Product Overview

Video Clip Extractor is a command-line tool that recursively scans directories for video files and automatically extracts short thematic clips (20-30 seconds) from each video. The tool helps users create preview thumbnails or theme videos for media libraries by intelligently selecting representative segments using configurable strategies (random or audio-based). 

Extracted clips are organized in `backdrops/` subdirectories with sequential naming (backdrop1.mp4, backdrop2.mp4, etc.). The tool supports incremental clip generation: if you run with `-c 2` to generate 2 clips and later run with `-c 3`, only the missing third clip will be generated, preserving existing clips.
