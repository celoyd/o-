/*

Coordinate conversion tool for working with Maxar ARDs. See help() for docs.

Alpha quality code. Not tested.

Todo:
- Testing.
- Test error message coverage and helpfulness.
- Conform function names/behaviors to rust conventions (as_*, from_*, etc.).

*/

use anyhow::{anyhow, ensure, Context, Result};
use proj::Proj;
use std::{env, fmt, process};

#[derive(PartialEq, Eq, Copy, Clone)]
enum Hemisphere {
    North,
    South,
}

impl Hemisphere {
    fn from_char(letter: &char) -> Result<Hemisphere> {
        match letter {
            'N' => Ok(Hemisphere::North),
            'S' => Ok(Hemisphere::South),
            _ => Err(anyhow!("Expected a hemisphere (N or S) but got {}.", letter)),
        }
    }

    fn from_lat(lat: &f64) -> Hemisphere {
        if *lat < 0.0 {
            Hemisphere::South
        } else {
            // includes NaN and infinities
            Hemisphere::North
        }
    }

    fn as_str(&self) -> &'static str {
        match self {
            Hemisphere::North => "N",
            Hemisphere::South => "S",
        }
    }

    fn as_proj(&self) -> &'static str {
        match self {
            Hemisphere::North => "",
            Hemisphere::South => "+south",
        }
    }
}

fn lonlat_to_utm_zone(lon: f64, lat: f64) -> (u8, Hemisphere) {
    let z: u8 = (((lon + 180.0) / 6.0).floor() + 1.0) as u8;
    let h: Hemisphere = Hemisphere::from_lat(&lat);
    (z, h)
}

struct UTMCoord {
    zone: u8,
    hemi: Hemisphere,
    x: f64,
    y: f64,
}

impl UTMCoord {
    fn new(zone: &u8, hemi: &Hemisphere, x: &f64, y: &f64) -> UTMCoord {
        UTMCoord {
            zone: *zone,
            hemi: *hemi,
            x: *x,
            y: *y,
        }
    }
}

impl fmt::Display for UTMCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{}{} {} {}",
            self.zone,
            self.hemi.as_str(),
            self.x as u64,
            self.y as u64
        )
    }
}

impl From<&LonLatCoord> for UTMCoord {
    fn from(source: &LonLatCoord) -> Self {
        let (zone, hemi) = lonlat_to_utm_zone(source.lon, source.lat);

        let src_proj: String = "+proj=lonlat".to_string();
        let dst_proj: String = format!("+proj=utm +zone={} {}", zone, hemi.as_proj());

        let utm_to_longlat = Proj::new_known_crs(&src_proj, &dst_proj, None).unwrap();
        let (utm_x, utm_y) = utm_to_longlat.convert((source.lon, source.lat)).unwrap();

        UTMCoord {
            zone,
            hemi,
            x: utm_x,
            y: utm_y,
        }
    }
}

struct LonLatCoord {
    lon: f64,
    lat: f64,
}

impl LonLatCoord {
    fn new(longitude: f64, latitude: f64) -> Result<Self, anyhow::Error> {
        if !(-180.0..=180.0).contains(&longitude) || !(-90.0..=90.0).contains(&latitude) {
            return Err(anyhow!(
                "Expected lon and lat in ranges (-180..180, -90..90) but got ({}, {})",
                longitude,
                latitude
            ));
        }

        Ok(LonLatCoord {
            lon: longitude,
            lat: latitude,
        })
    }

    fn as_deluxe_string(&self) -> String {
        let tidy_lon: String = format!("{:.5}", self.lon);
        let tidy_lat: String = format!("{:.5}", self.lat);

        format!("Lon, lat: {tidy_lon}, {tidy_lat}\nLat/lon: {tidy_lat}/{tidy_lon}")
    }
}

impl fmt::Display for LonLatCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:.5}, {:.5}", self.lon, self.lat)
    }
}

impl From<&UTMCoord> for LonLatCoord {
    fn from(source: &UTMCoord) -> Self {
        let src_proj: String = format!("+proj=utm +zone={} {}", source.zone, source.hemi.as_proj());
        let dst_proj: String = "+proj=lonlat".to_string();

        let lonlat_to_utm = match Proj::new_known_crs(&src_proj, &dst_proj, None) {
            Ok(transform) => transform,
            Err(e) => panic!("Proj failed to make a transformer from “{}” to “{}”: {}", src_proj, dst_proj, e)
        };

        let (lon, lat) = match lonlat_to_utm.convert((source.x, source.y)) {
            Ok((x, y)) => (x, y),
            Err(e) => panic!("Proj failed to convert: {}", e)
        };

        match LonLatCoord::new(lon, lat) {
            Ok(ll) => { ll },
            Err(e) => {
                panic!("Lon/lat out of bounds: {}", e)
            }
        }
    }
}

#[derive(Copy, Clone)]
struct MGSCoord {
    zone: u8,
    key: [u8; 12],
}

impl MGSCoord {
    fn key_to_string(&self) -> String {
        self.key.into_iter().map(|c| c.to_string()).collect()
    }

    fn from_u8_and_str(zone: u8, key_string: &str) -> MGSCoord {
        let mut key: [u8; 12] = [0; 12];
        for (i, c) in key_string.chars().enumerate() {
            key[i] = c.to_digit(10).expect("Quadkey digit not in 0..3!") as u8;
        }
        MGSCoord { zone, key }
    }
}

impl fmt::Display for MGSCoord {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}/{}", self.zone, self.key_to_string())
    }
}

impl From<&UTMCoord> for MGSCoord {
    fn from(source: &UTMCoord) -> Self {
        // First we knock off false easting and northing:
        let mut x: f64 = source.x;
        x -= 500_000.0;
        let mut y: f64 = source.y;

        if source.hemi == Hemisphere::South {
            y -= 10_000_000.0
        }

        // Now x and y are in meters from the zone centerpoint.
        // Next we put everything in terms of 5 km grid cells.

        x /= 5_000.0;
        y /= 5_000.0;

        // set up the grid
        let ix = (x + 2048.0) as i16;
        let iy = (2048.0 - y) as i16;

        // let mut qk = vec![0_u8; 12];
        let mut key: [u8; 12] = [0; 12];
        let mut mask: i16;

        for (z, c) in key.iter_mut().enumerate() {
            mask = 1 << (11 - z);
            if ix & mask > 0 {
                *c += 1
            }
            if iy & mask > 0 {
                *c += 2
            }
        }

        MGSCoord {
            zone: source.zone,
            key,
        }
    }
}

impl From<&MGSCoord> for UTMCoord {
    fn from(source: &MGSCoord) -> Self {
        let mut ix: i16 = 0;
        let mut iy: i16 = 0;

        let mut mask: i16;

        for (z, c) in source.key.iter().enumerate() {
            mask = 1 << (11 - z);
            match c {
                0 => {}
                1 => ix |= mask,
                2 => iy |= mask,
                3 => {
                    ix |= mask;
                    iy |= mask;
                }
                _ => {
                    panic!("Quadkey digit not in 0..3!")
                }
            }
        }

        // Now ix and iy are in MGS cell space. Next, convert to m.

        let mut x: f64 = ix as f64;
        let mut y: f64 = iy as f64;

        // move origin to zone center
        x -= 2048.0;
        y = 2048.0 - y;

        // not really a lat, but has the same sign
        let hemi: Hemisphere = Hemisphere::from_lat(&y);

        // scale by grid size
        x *= 5_000.0;
        y *= 5_000.0;

        // We are now in meters. Next, inject false easting and northing.
        x += 500_000.0;
        if y < 0.0 {
            y += 10_000_000.0
        }

        UTMCoord {
            zone: source.zone,
            hemi,
            x,
            y,
        }
    }
}

fn help() {
    eprintln!(
        "\
oö is a utility for coördinate conversions useful with Maxar ARD. Give it \
any of WGS84 (lon, lat), UTM, or Maxar Grid System coördinates to get all \
three.

Usage (matched by argument count):
oö <MGS cell in ZZ/QKQKQKQKQKQK format>
oö <longitude> <latitude>
oö <UTM zone> <easting> <northing>

Example:
$ oö -99.09357951534054 19.29675919163688
Lon, lat: -99.09358, 19.29676
Lat/lon: 19.29676/-99.09358
UTM 14N 490168 2133666
14/033113131312

Conventions:
1. MGS cells are treated as their centers in conversions, and are read and \
written only at level 12.
2. WGS84 (lon/lat) and UTM coordinates are read at any (float64) precision \
but written at ~1 meter precision (integer for UTM, 5 decimals for WGS84).
3. MGS and UTM coördinates are read in any zone but written in their \
canonical zone."
    );
}

fn make_message(ll: LonLatCoord) -> Result<String, anyhow::Error> {
    let utm = UTMCoord::from(&ll);
    let mgs = MGSCoord::from(&utm);

    Ok(format!("{}\n{}\n{}", ll.as_deluxe_string(), utm, mgs))
}

fn from_ll(argv: Vec<String>) -> Result<String> {
    let lon = argv[1].parse::<f64>().context(format!(
        "Expected a numeric longitude but got “{}”.",
        argv[1]
    ))?;
    let lat = argv[2].parse::<f64>().context(format!(
        "Expected a numeric latitude but got “{}”.",
        argv[2]
    ))?;

    let ll = LonLatCoord::new(lon, lat).context(format!(
        "Not a geographically sensible longitude and latitude: {}, {}.",
        lon, lat
    ))?;

    make_message(ll)
}

fn from_mgs(argv: Vec<String>) -> Result<String, anyhow::Error> {
    let (z, c) = argv[1]
        .split_once('/')
        .context("With one argument, expected an MGS tile like 42/012301230123, with the slash.")?;
    let zone = z
        .parse::<u8>()
        .context(format!("Expected a zone in 01..60 but got {}.", z))?;

    ensure!(
        c.len() == 12,
        format!(
            "Expected a quadkey of length 12 but got “{}” (length {}).",
            c,
            c.len()
        )
    );

    // Please see the UTM normalization comment in from_utm().
    let mgs: MGSCoord = MGSCoord::from_u8_and_str(zone, c);
    let utm = UTMCoord::from(&mgs);
    let ll = LonLatCoord::from(&utm);

    make_message(ll)
}

fn from_utm(argv: Vec<String>) -> Result<String> {
    let zone_string = &argv[1];

    ensure!(
        (1..=3).contains(&zone_string.len()),
        format!(
            "Expected a UTM zone like 1, 23N, or 42S, but got {}.",
            zone_string
        )
    );

    let last_character = zone_string.chars().last().unwrap();
    let (zone, hemi) = match last_character {
        'N' | 'S' => (
            zone_string[..zone_string.len() - 1]
                .parse::<u8>()
                .context(format!(
                    "Expected an integer UTM zone (with optional N/S), but got {}.",
                    zone_string
                ))?,
            Hemisphere::from_char(&last_character).unwrap(), // infallible given match
        ),
        _ => (
            zone_string.parse::<u8>().context(format!(
                "Expected a UTM zone like 1, 23N, or 42S, but got {}.",
                zone_string
            ))?,
            Hemisphere::North,
        ),
    };

    let x = argv[2]
        .parse::<f64>()
        .context(format!("Expected numeric UTM easting but got {}", argv[2]))?;
    let y = argv[3]
        .parse::<f64>()
        .context(format!("Expected numeric UTM northing but got {}", argv[3]))?;

    /*
    # UTM normalization

    We’re about to do UTM -> lonlat here, then pass the lonlat to
    make_message, which will do the reverse. This is deliberate, to get
    the canonical/normalized zone for UTM (and MGS, which is based on it).
    It’s a debatable choice, so let’s explore the considerations a bit.

    Usually it makes sense to think of UTM as giving us a 1:1 correspondence
    of (lon, lat) <-> (zone, easting, northing). But when operating around
    zone edges, for example, it can be convenient to pick one reference zone
    and represent all coordinates in its terms, even if they canonically
    belong to the other zone. If done carefully, with an understanding of
    the convenience v. distortion costs, this is a useful pattern and should
    be understood as a practical affordance of UTM, not as a problem. But it
    adds complexity, since we don’t know the user’s intent.

    ## Example

    This is a UTM issue, but MGS (built on top of UTM) makes a more useful
    example than raw UTM would.

    Suppose we’re working with MGS grid cell 47/122021022202 and we want to
    look at the cell to its east. Using the quadkey system, we know that we
    can get there by changing the 2 at the end into a 3. However, this has
    taken us across a UTM zone boundary: 47/122021022203 is a non-canonical
    coordinate and (I confidently assume) the Maxar ARD system will not
    produce a tile with this name.

    There are plenty of sensible things to do with a non-canonical coordinate.
    We could panic, we could explain the problem to the user, and so on. But
    the route taken here is to silently canonicalize. We assume that a user
    who doesn’t want this will be paying enough attention to notice it.

    This means we convert the centerpoint of 47/122021022203 to UTM, take
    that to WGS84 (lon, lat), and then turn around: -> UTM -> MGS, where it
    will come out as 48/033131022202, which is canonical.

    ## Drawbacks of this approach

    1. Because the grids of different zones differ, there’s an unavoidable
       shift in centerpoints. The center of 47/122021022003 is about 2.9 km
       (more than half a cell-width!) from the center of 48/033131022202. So
       we have to remember that these are not merely two names for one cell;
       we’re really going cell -> point -> different cell. While we can think
       of this as simple reprojection in UTM terms, the MGS cells introduce
       a layer that can cause conceptual errors.

    2. We give up on catching many likely-bad coordinates. For example, the
       odds that someone wants to address somewhere canonically in zone 47
       from zone 1 are practically nil. If they really do, why with this
       tiny, purpose-built utility? Much more likely it’s a typo. But we
       don’t even warn them; we take the bonkers coordinate and silently
       canonicalize it into zone 47.

    This is the behavior that I want for my very specific purposes, but
    anyone building on my work should understand that it’s a tricky question
    and the best answer for them may be different.
    */

    let utm = UTMCoord::new(&zone, &hemi, &x, &y);
    let ll = LonLatCoord::from(&utm);
    make_message(ll)
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.contains(&String::from("--help")) {
        help();
        process::exit(0)
    }

    let message = match args.len() - 1 {
        1 => from_mgs(args),
        2 => from_ll(args),
        3 => from_utm(args),
        _ => Err(anyhow!(
            "Expected 1 argument (MGS coord), 2 (lon lat), or 3 (UTM), but got {}.\nSee --help.",
            args.len() - 1
        )),
    };

    match message {
        Ok(m) => {
            println!("{}", m)
        }
        Err(e) => {
            eprintln!("{}", e);
            process::exit(1)
        }
    }
}
