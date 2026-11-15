# The Grant logo

The mark is drawn with the vocabulary of the page's figures: a circle is a world, a dot is an
object, an arrow is a fact. A world left open, with one fact pointing at the object inside
it, reads as the letter G.

| File | Use |
| --- | --- |
| `grant-mark.svg`, `grant-mark-white.svg` | the mark alone, on light and on dark |
| `grant-logo.svg`, `grant-logo-white.svg` | the mark with the name, set in Computer Modern and stored as outlines, so no font is needed |
| `favicon.svg` | the mark drawn heavier for small sizes; it turns light when the browser is in dark mode |
| `favicon-32.png`, `favicon-180.png` | the same for browsers and devices that want a picture |
| `grant-mark-512.png` | the mark as a picture |

## Construction

On a circle of radius 40 centred on the origin:

- the world is the arc from 34° above the horizontal, counter-clockwise, round to the point (40, 0);
- the fact is the segment from (40, 0) to (11.5, 0), ending in an open arrowhead 8.2 long and 10.6 wide;
- the object is a disc of radius 3.7 at the origin;
- every line is 2 wide.

Leave clear space of at least the radius of the mark on every side. The mark is one colour:
ink on paper, or paper on ink.

For the page, add to the head of `index.html`, or to the `html-head-favicon` entry of `index.tm`:

```html
<link rel="icon" href="favicon.svg" type="image/svg+xml">
<link rel="apple-touch-icon" href="favicon-180.png">
```
