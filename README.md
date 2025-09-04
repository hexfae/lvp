# pixelfLut Video Processor

originally [ludd](https://ludd.ltu.se/) video processor

processes videos for pixelflut, specifically [pixelpwnr](https://github.com/timvisee/pixelpwnr-server)
servers (since those (by default) have the "binary PX" command , PB, which this uses exclusively)

## Features

- processes regular video files just-in-time (specifically 640x360 ones) using ffmpeg, no pre-processing
needed (besides resizing to 640x360)
  - can change the assortment of videos at run-time, just add/remove files from the selected directory
- uses [pixelpwnr's](https://github.com/timvisee/pixelpwnr-server) binary pixel command to reduce bandwidth
- caches unchanged pixels to not waste bandwidth on duplicate pixels
- drops frames to catch up when lagging behind
- 100% documented, small codebase (currently <500 sloc), that aims to be relatively simple/understandable

with all that said, it can send 640x360 60fps videos to the [ludd](https://ludd.ltu.se/) with nearly
zero dropped frames (assuming there isn't too much changing on every frame) and only uses like 400Mbps

## Running

you need a file called `static.mp4` file in the chosen directory, which will play for 1 second between
videos. you also need at least 2 non-`static.mp4` videos, which it will randomly choose between and play

## Building

`cargo build [--release]`

## To-do list

- [ ] A Web UI?

## License

`AGPL-3.0-or-later`, to allow for creating a web ui in the future

## Contributing

Please contribute

## Is it any good?

Yes.
