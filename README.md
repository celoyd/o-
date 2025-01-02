# oö
Tiny Rust CLI utility for MGS ARD coördinate conversion

This is a small tool to convert among WGS84 (longitude and latitude), UTM, and the [Maxar ARD grid system](https://ard.maxar.com/docs/sdk/sdk/ard_grid/). It was written to meet a trivial personal need and is very simple. It’s also the first Rust I’ve ever written (beyond hello-world demos) and is not well tested, so it should be treated as early beta quality at best.

## Background

Sometimes I’m looking at something I pansharpened in QGIS in a UTM projection and want the WGS84 coordinates of what I’m looking at. I’m sure there’s a one-click way to do this in QGIS, but I _also_ sometimes want to know the rough location of an image just from its grid cell (without `rio bounds`), and I _also_ sometimes want to know what grid cell a certain WGS coördinate would fall in so I can look something up, and all of these _together_ seemed to merit a little tool that just did all the conversions.

It’s called oö after the fancy way of spelling coördinates with a [diaeresis](https://en.wikipedia.org/wiki/Diaeresis_(diacritic)). That is the official spelling within this project.

## Usage and example

The tool understands three coordinate systems, which it tells apart only by how many command line arguments it sees.

1. One argument: An MGS grid cell in the format ZZ/QKQKQKQKQKQK, for example `19/213133023133`.
2. Two arguments are parsed as longitude, latitude (in that order – don’t @ me), for example `-122.667 45.505`.
3. Three arguments are parsed as UTM zone, easting, and northing, for example `56S 334871 6252376`.

Given any of these (that it can parse), it emits all three, with the longitude and latitude in both orders for easy copying and pasting. For example:

```bash
$ oö 56S 334871 6252316
Lon, lat: 151.21499, -33.85655
Lat/lon: -33.85655/151.21499
56S 334870 6252316
56/213133213312
```

For more, see the `--help`. There’s also a lengthy comment about UTM normalization (which it does, but which you might not want).

## Contributing

Unlikely.
