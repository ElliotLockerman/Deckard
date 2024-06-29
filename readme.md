
![Header](media/header.png)

Deckard is a Rust program for finding duplicate images. It recursively walks a file system, computing a perceptual hash of each image it finds, and displaying sets of images with the same hash. Using a perceptual hash allows it to find matching images even after being resized or lightly modified.

![Screenshots](media/screenshots.png)

# Building

`cargo build` works fine.

On Mac OS, you can also use `build.sh`; this runs `cargo build`, then `/scripts/make-app.sh`, which builds `Deckard.app` (in `target/{BUILD_MODE}/Deckard.app`).

