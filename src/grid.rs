use ratatui::layout::Rect;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Placement {
    pub column: u16,
    pub row: u16,
    pub column_span: u16,
    pub row_span: u16,
}

impl Placement {
    pub fn overlaps(self, other: Self) -> bool {
        u32::from(self.column) < u32::from(other.column) + u32::from(other.column_span)
            && u32::from(other.column) < u32::from(self.column) + u32::from(self.column_span)
            && u32::from(self.row) < u32::from(other.row) + u32::from(other.row_span)
            && u32::from(other.row) < u32::from(self.row) + u32::from(self.row_span)
    }

    pub fn fits(self, columns: u16, rows: u16) -> bool {
        self.column_span > 0
            && self.row_span > 0
            && u32::from(self.column) + u32::from(self.column_span) <= u32::from(columns)
            && u32::from(self.row) + u32::from(self.row_span) <= u32::from(rows)
    }
}

/// Integer boundaries distribute leftover terminal cells without gaps or overlaps.
#[derive(Clone, Copy)]
pub struct Grid {
    pub area: Rect,
    pub columns: u16,
    pub rows: u16,
}

impl Grid {
    pub fn rect(self, p: Placement) -> Rect {
        let boundary = |index: u16, count: u16, length: u16| -> u16 {
            (u32::from(index.min(count)) * u32::from(length) / u32::from(count.max(1))) as u16
        };
        let x = boundary(p.column, self.columns, self.area.width);
        let y = boundary(p.row, self.rows, self.area.height);
        let right = boundary(
            p.column.saturating_add(p.column_span),
            self.columns,
            self.area.width,
        );
        let bottom = boundary(
            p.row.saturating_add(p.row_span),
            self.rows,
            self.area.height,
        );
        Rect::new(self.area.x + x, self.area.y + y, right - x, bottom - y)
    }

    pub fn cell(self, x: u16, y: u16) -> Option<(u16, u16)> {
        if !self.area.contains((x, y).into()) || self.area.is_empty() {
            return None;
        }
        // Invert the rounded boundaries, including grids smaller than their cell count.
        let column = (((u32::from(x - self.area.x) + 1) * u32::from(self.columns) - 1)
            / u32::from(self.area.width)) as u16;
        let row = (((u32::from(y - self.area.y) + 1) * u32::from(self.rows) - 1)
            / u32::from(self.area.height)) as u16;
        Some((column, row))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn touching_edges_are_not_collisions_and_overflows_are_rejected() {
        let a = Placement {
            column: 0,
            row: 0,
            column_span: 4,
            row_span: 2,
        };
        let b = Placement {
            column: 4,
            row: 0,
            column_span: 2,
            row_span: 1,
        };
        assert!(!a.overlaps(b));
        assert!(a.overlaps(Placement { column: 3, ..b }));
        assert!(
            !Placement {
                column: u16::MAX,
                ..a
            }
            .fits(6, 4)
        );
        assert!(
            !Placement {
                column_span: 0,
                ..a
            }
            .fits(6, 4)
        );
    }

    #[test]
    fn every_rendered_cell_maps_back_to_its_grid_coordinate() {
        for width in 1..60 {
            let grid = Grid {
                area: Rect::new(2, 3, width, 19),
                columns: 6,
                rows: 4,
            };
            for row in 0..grid.rows {
                for column in 0..grid.columns {
                    let rect = grid.rect(Placement {
                        column,
                        row,
                        column_span: 1,
                        row_span: 1,
                    });
                    for y in rect.y..rect.bottom() {
                        for x in rect.x..rect.right() {
                            assert_eq!(grid.cell(x, y), Some((column, row)));
                        }
                    }
                }
            }
        }
    }
}
