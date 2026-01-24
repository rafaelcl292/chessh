# 8-Bit Chess Assets

This folder contains pixel art assets for a chess game.

**Source:** https://cosunosuke.itch.io/8-bit-chess

## Files

- `pieces.png` - Chess piece sprites (28 sprites total)
- `board.png` - Chess board texture

## board.png

**Image Dimensions:** 100×100 pixels

A complete chess board texture with an 8×8 grid of alternating light and dark squares, with a decorative border around the edges.

## pieces.png

**Image Dimensions:** 70×40 pixels (7 columns × 4 rows = 28 sprites total)

**Individual Sprite Size:** 10×10 pixels per sprite

**Sprite Layout:**

### Rows 1-4: Chess Pieces (6 pieces per color × 4 colors = 24 sprites)

Each row contains: `pawn`, `knight`, `bishop`, `rook`, `queen`, `king`, `black_square`

**Row 1:** Brown pieces  
**Row 2:** Yellow pieces  
**Row 3:** Dark blue pieces  
**Row 4:** Light blue pieces

### Column 7 (Board Squares):

- Row 1: `black_square`
- Row 2: `white_square`
- Row 3: `black_square` with checker piece
- Row 4: `white_square` with checker piece
