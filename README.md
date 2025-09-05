# pixelfLut Video Processor

originally [ludd](https://ludd.ltu.se/) video processor

processes videos and sends frames to pixelflut servers

## Features

- processes regular video files just-in-time (specifically 640x360 ones) using ffmpeg, no pre-processing
  needed (besides resizing to 640x360), and displays them in the bottom-right of 1920x1080 pixelflut servers
  - can change the assortment of videos at run-time, just add/remove files from the selected directory
- picks a random timestamp of a random video, plays it for 10 seconds, then switches to a new one with
  1 second of tv static in between
- supports both standard text pixel commands (`PX`) as specified by the pixelflut standard as well as
  [pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server)'s binary pixel command (`PB`) for
  significantly reduced bandwidth usage (~30-50%)
- uses tokio tasks in order to process video frames and send them to the server in parallel
- caches unchanged pixels as to not waste bandwidth on duplicate pixels
- reduced color resolution of videos by 1/10th in order to increase the effectiveness of the cache
  (barely noticeable on large screens at a far distance)
- drops frames as needed in order to catch up when lagging behind
- heavily optimized code, aims to be as as light as reasonably possible on both cpu and memory
- 100% documented, small codebase (currently <400 sloc), that aims to be relatively simple/understandable
- 🔥🚀🦀

## Usage

`lvp --address <ADDRESS> --directory <DIRECTORY> [--binary]`

`ADDRESS` should be an address running an implemention of a pixelflut server, `DIRECTORY` needs to point to
an applicable directory (read below), `binary` is recommended if the server is running
[pixelpwnr-server](https://github.com/timvisee/pixelpwnr-server)

you need a file called `static.mp4` file in the chosen directory, which will play for 1 second between
videos. you also need at least 1 non-`static.mp4` video, which it will randomly choose between and play

## Building

`cargo build [--release]`

there is a nix flake available, which provides outputs for the `nix build`, `nix run`, and `nix develop` commands

for folks not using nix[os], you'll probably need ffmpeg libraries and clang and maybe pkg-config in order to
compile, idk check the `buildInputs` and `nativeBuildInputs` of the `flake.nix` file

## To-do list

- [ ] A Terminal UI?
- [ ] A Web UI?

## License

`AGPL-3.0-or-later`, to allow for creating a web ui in the future

## Contributing

Please contribute

## Is it any good?

Yes.
