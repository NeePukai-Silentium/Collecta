//! Where each tile of the feed goes: an even grid of square cells, or masonry
//! (columns of equal width, each picture keeps its proportions and goes to the shortest column).

#[derive(Clone, Copy, PartialEq)]
pub struct Params {
    pub width: f32,
    pub thumb: f32,
    /// Height of the name and size under a tile, 0 in the compact mode.
    pub caption: f32,
    pub gap: f32,
    pub masonry: bool,
}

pub struct Placed {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

/// `ratios` — width / height of each picture. Returns the tiles and the total height.
pub fn layout(ratios: &[f32], params: &Params) -> (Vec<Placed>, f32) {
    let Params { width, thumb, caption, gap, masonry } = *params;
    let columns = (((width + gap) / (thumb + gap)).floor() as usize).max(1);
    let cell = ((width - gap * (columns - 1) as f32) / columns as f32).max(1.0);
    let mut bottoms = vec![0.0f32; columns];
    let mut placed = Vec::with_capacity(ratios.len());

    for (index, ratio) in ratios.iter().enumerate() {
        let (column, height) = if masonry {
            let shortest = (0..columns).min_by(|a, b| bottoms[*a].total_cmp(&bottoms[*b])).unwrap_or(0);
            (shortest, cell / ratio.clamp(0.3, 4.0) + caption)
        } else {
            (index % columns, cell + caption)
        };
        placed.push(Placed { x: column as f32 * (cell + gap), y: bottoms[column], width: cell, height });
        bottoms[column] += height + gap;
    }

    let total = bottoms.iter().copied().fold(0.0, f32::max);
    (placed, (total - gap).max(0.0))
}
