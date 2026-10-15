//! Computer-vision fiducial geometry for the `diag` page (layout v3, revision 1).
//!
//! Everything here is procedural: no image is stored and no vision library is linked.
//!
//! * **ArUco 4x4** (`DICT_4X4_50`, OpenCV-compatible ids and bit patterns). The dictionary table below
//!   holds the 16 payload bits of ids 0..=19. Derivation / source: the OpenCV predefined dictionary
//!   `DICT_4X4_50` (`cv::aruco::getPredefinedDictionary`); the value of id `n` is the rotation-0 payload
//!   (`bytesList[n][0..2]`, big endian) which equals the inner 4x4 modules of
//!   `Dictionary.generateImageMarker(n, 6)` read row-major from the top-left with white = 1. Example: id 0
//!   is bytes 181, 50 = `0xB532`. A marker is a 6x6 module square: a one-module black border around the
//!   4x4 payload. The Python tests compare every table entry against `cv2` when it is importable.
//! * **Chessboard** (black top-left square), **ChArUco** (OpenCV >= 4.6 non-legacy layout: square (0,0) is
//!   black, markers sit in the squares with odd `row + col`, ids run row-major over those squares starting
//!   at the first id) and the OpenCV **asymmetric circle grid** (`x = (2 j + i % 2) s`, `y = i s`).
//! * Black / white corner **L-brackets** with asymmetric marks.
//!
//! Coordinates are page millimetres (origin top-left, y down) like the rest of `diag_v3`.

pub type R4 = [f64; 4];

pub const ARUCO_DICT_NAME: &str = "DICT_4X4_50";
/// Payload of ids 0..=19, row-major, MSB = top-left module, 1 = white module.
pub const ARUCO_4X4_50: [u16; 20] = [
    0xB532, 0x0F9A, 0x332D, 0x9946, 0x549E, 0x79CD, 0x9E2E, 0xC4F2, 0xFEDA, 0xCF56, 0xF991, 0x11A7,
    0x0EB7, 0x2A0F, 0x24B1, 0x263E, 0x4665, 0x6600, 0x6C5E, 0x76AF,
];
/// Number of ids this generator can draw.
pub const ARUCO_IDS: usize = ARUCO_4X4_50.len();
/// Smallest drawn module of any ArUco marker, millimetres (about 4 px at 100 dpi).
pub const MIN_MODULE_MM: f64 = 1.0;
/// Smallest chessboard / ChArUco square and circle-grid spacing, millimetres.
pub const MIN_SQUARE_MM: f64 = 2.4;

/// The 6x6 modules of marker `id` (`true` = black). Border black; payload bit 1 = white.
pub fn aruco_cells(id: usize) -> [[bool; 6]; 6] {
    let code = ARUCO_4X4_50[id % ARUCO_IDS];
    let mut g = [[true; 6]; 6];
    for r in 0..4 {
        for c in 0..4 {
            let white = (code >> (15 - (r * 4 + c))) & 1 == 1;
            g[r + 1][c + 1] = !white;
        }
    }
    g
}

/// Corner points of a square marker rect in OpenCV order: TL, TR, BR, BL (canonical orientation).
pub fn corners_of(r: R4) -> [[f64; 2]; 4] {
    [[r[0], r[1]], [r[2], r[1]], [r[2], r[3]], [r[0], r[3]]]
}

/// Maximal black rectangles of a module grid, merged first along rows then down identical runs.
pub fn grid_rects(cells: &[Vec<bool>], x0: f64, y0: f64, cell: f64) -> Vec<R4> {
    let mut runs: Vec<(usize, usize, usize, usize)> = vec![]; // (row0, row1_excl, col0, col1_excl)
    let mut open: Vec<(usize, usize, usize)> = vec![]; // (row0, col0, col1) still extending
    for (ri, row) in cells.iter().enumerate() {
        let mut cur: Vec<(usize, usize)> = vec![];
        let mut c = 0;
        while c < row.len() {
            if row[c] {
                let s = c;
                while c < row.len() && row[c] {
                    c += 1;
                }
                cur.push((s, c));
            } else {
                c += 1;
            }
        }
        let mut next_open = vec![];
        for (r0, c0, c1) in open.drain(..) {
            if cur.contains(&(c0, c1)) {
                cur.retain(|x| *x != (c0, c1));
                next_open.push((r0, c0, c1));
            } else {
                runs.push((r0, ri, c0, c1));
            }
        }
        for (c0, c1) in cur {
            next_open.push((ri, c0, c1));
        }
        open = next_open;
    }
    for (r0, c0, c1) in open {
        runs.push((r0, cells.len(), c0, c1));
    }
    runs.sort();
    runs.into_iter()
        .map(|(r0, r1, c0, c1)| {
            [
                x0 + c0 as f64 * cell,
                y0 + r0 as f64 * cell,
                x0 + c1 as f64 * cell,
                y0 + r1 as f64 * cell,
            ]
        })
        .collect()
}

/// Black rects of ArUco marker `id` drawn at `r` (a square).
pub fn aruco_rects(id: usize, r: R4) -> Vec<R4> {
    let g = aruco_cells(id);
    let cells: Vec<Vec<bool>> = g.iter().map(|row| row.to_vec()).collect();
    grid_rects(&cells, r[0], r[1], (r[2] - r[0]) / 6.0)
}

/// Marker rows as strings, `1` = black module.
pub fn aruco_rows(id: usize) -> Vec<String> {
    aruco_cells(id)
        .iter()
        .map(|row| row.iter().map(|&v| if v { '1' } else { '0' }).collect())
        .collect()
}

/// Chessboard of `cols x rows` squares of `sq` mm with the top-left square black.
/// Returns the black square rects and the inner corner positions (row-major, `(cols-1) x (rows-1)`).
pub fn chessboard(x0: f64, y0: f64, cols: usize, rows: usize, sq: f64) -> (Vec<R4>, Vec<[f64; 2]>) {
    let mut black = vec![];
    for r in 0..rows {
        for c in 0..cols {
            if (r + c) % 2 == 0 {
                black.push([
                    x0 + c as f64 * sq,
                    y0 + r as f64 * sq,
                    x0 + (c + 1) as f64 * sq,
                    y0 + (r + 1) as f64 * sq,
                ]);
            }
        }
    }
    let mut inner = vec![];
    for r in 1..rows {
        for c in 1..cols {
            inner.push([x0 + c as f64 * sq, y0 + r as f64 * sq]);
        }
    }
    (black, inner)
}

/// ChArUco board (OpenCV non-legacy): plain black squares where `row + col` is even, a marker of side
/// `marker` centred in every square with odd `row + col`. `first_id` numbers the markers row-major.
pub struct Charuco {
    pub black_squares: Vec<R4>,
    /// (id, marker rect) in id order.
    pub markers: Vec<(usize, R4)>,
    /// Chessboard inner corners row-major `(cols-1) x (rows-1)`; ChArUco corner ids follow this order.
    pub corners: Vec<[f64; 2]>,
}

pub fn charuco(
    x0: f64,
    y0: f64,
    cols: usize,
    rows: usize,
    sq: f64,
    marker: f64,
    first_id: usize,
) -> Charuco {
    let mut black_squares = vec![];
    let mut markers = vec![];
    let mut id = first_id;
    for r in 0..rows {
        for c in 0..cols {
            let (sx, sy) = (x0 + c as f64 * sq, y0 + r as f64 * sq);
            if (r + c) % 2 == 0 {
                black_squares.push([sx, sy, sx + sq, sy + sq]);
            } else {
                let o = (sq - marker) / 2.0;
                markers.push((id, [sx + o, sy + o, sx + o + marker, sy + o + marker]));
                id += 1;
            }
        }
    }
    let mut corners = vec![];
    for r in 1..rows {
        for c in 1..cols {
            corners.push([x0 + c as f64 * sq, y0 + r as f64 * sq]);
        }
    }
    Charuco {
        black_squares,
        markers,
        corners,
    }
}

/// OpenCV asymmetric circle grid: `per_row` points in each of `rows` rows, `sp` = row pitch;
/// point `(i, j)` is at `x0 + (2 j + i % 2) sp`, `y0 + i sp`. Row-major order (OpenCV object points).
pub fn circle_grid(x0: f64, y0: f64, per_row: usize, rows: usize, sp: f64) -> Vec<[f64; 2]> {
    let mut v = vec![];
    for i in 0..rows {
        for j in 0..per_row {
            v.push([x0 + (2 * j + i % 2) as f64 * sp, y0 + i as f64 * sp]);
        }
    }
    v
}

/// Corner L-bracket at a corner. `(sx, sy)` = direction from the elbow into the sheet (+1 / -1).
/// Returns the black rects, the white mark rect (inside the horizontal arm) and the black inner-corner
/// mark rect. `idx` (0..3) moves the white mark along the arm so every corner differs.
pub struct Bracket {
    pub black: Vec<R4>,
    pub white_mark: R4,
    pub black_mark: R4,
}

pub const BRACKET_ARM_MM: f64 = 9.0;
pub const BRACKET_THICK_MM: f64 = 2.4;
pub const BRACKET_MARK_MM: f64 = 0.9;

pub fn bracket(ex: f64, ey: f64, sx: f64, sy: f64, idx: usize) -> Bracket {
    let (a, t, m) = (BRACKET_ARM_MM, BRACKET_THICK_MM, BRACKET_MARK_MM);
    let span = |p: f64, s: f64, d0: f64, d1: f64| {
        let (u, v) = (p + s * d0, p + s * d1);
        (u.min(v), u.max(v))
    };
    let rect = |dx0: f64, dx1: f64, dy0: f64, dy1: f64| {
        let (x0, x1) = span(ex, sx, dx0, dx1);
        let (y0, y1) = span(ey, sy, dy0, dy1);
        [x0, y0, x1, y1]
    };
    let off = 4.0 + 1.2 * idx as f64;
    Bracket {
        black: vec![rect(0.0, a, 0.0, t), rect(0.0, t, t, a)],
        white_mark: rect(off, off + m, (t - m) / 2.0, (t + m) / 2.0),
        black_mark: rect(t + 1.2, t + 1.2 + m, t + 1.2, t + 1.2 + m),
    }
}

/// Ruler tick table: (index, position mm from the zero line, length mm), for `count + 1` ticks at
/// `pitch` where the longest tier that divides the index wins.
pub fn ruler_ticks(pitch: f64, count: u32, tiers: &[(u32, f64)]) -> Vec<(u32, f64, f64)> {
    let mut v = vec![];
    for i in 0..=count {
        let len = tiers
            .iter()
            .rev()
            .find(|(every, _)| i % every == 0)
            .map(|t| t.1)
            .unwrap_or(0.0);
        if len > 0.0 {
            v.push((i, i as f64 * pitch, len));
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dictionary_is_the_documented_one() {
        assert_eq!(ARUCO_4X4_50[0], u16::from_be_bytes([181, 50]));
        assert_eq!(ARUCO_4X4_50[1], u16::from_be_bytes([15, 154]));
        assert_eq!(ARUCO_4X4_50[11], u16::from_be_bytes([17, 167]));
        let mut s = ARUCO_4X4_50.to_vec();
        s.sort();
        s.dedup();
        assert_eq!(s.len(), ARUCO_IDS);
        // border all black
        for id in 0..ARUCO_IDS {
            let g = aruco_cells(id);
            assert!(g[0].iter().chain(g[5].iter()).all(|&v| v));
            assert!(g.iter().all(|row| row[0] && row[5]));
        }
    }

    #[test]
    fn rect_merge_covers_exactly_the_black_cells() {
        for id in 0..ARUCO_IDS {
            let rs = aruco_rects(id, [0.0, 0.0, 6.0, 6.0]);
            let g = aruco_cells(id);
            for (r, row) in g.iter().enumerate() {
                for (c, &black) in row.iter().enumerate() {
                    let (x, y) = (c as f64 + 0.5, r as f64 + 0.5);
                    let hits = rs
                        .iter()
                        .filter(|q| x > q[0] && x < q[2] && y > q[1] && y < q[3])
                        .count();
                    assert_eq!(hits, black as usize, "id {id} cell {r},{c}");
                }
            }
        }
    }

    #[test]
    fn charuco_follows_the_opencv_layout() {
        let b = charuco(0.0, 0.0, 6, 4, 5.0, 3.5, 8);
        assert_eq!(b.markers.len(), 12);
        assert_eq!(b.markers[0].0, 8);
        // first marker sits in square (row 0, col 1)
        assert!((b.markers[0].1[0] - 5.75).abs() < 1e-9);
        assert_eq!(b.corners.len(), 5 * 3);
        assert_eq!(b.black_squares.len(), 12);
    }
}
