import {
  DEFAULT_STAMP_WIDTH,
  STAMP_BOX,
  fillColor,
  stampByKey,
  stampPalette,
} from "./stamp-library";

/**
 * The DOM (SVG) drawing of a stamp, from the same data as the canvas
 * renderer, so previews in the palette and properties panel match the map.
 */
export function StampSvg({
  icon,
  color = null,
  size = 40,
  className,
}: {
  icon: string;
  color?: string | null;
  size?: number;
  className?: string;
}) {
  const def = stampByKey(icon);
  const palette = stampPalette(color);
  return (
    <svg
      viewBox={`0 0 ${STAMP_BOX} ${STAMP_BOX}`}
      width={size}
      height={size}
      className={className}
      aria-hidden="true"
      focusable="false"
    >
      {def ? (
        def.shapes.map((shape, i) => (
          <path
            key={i}
            d={shape.d}
            fill={shape.fill === "none" ? "none" : fillColor(shape.fill, palette)}
            stroke={shape.w === 0 ? "none" : palette.ink}
            strokeWidth={shape.w ?? DEFAULT_STAMP_WIDTH}
            strokeLinejoin="round"
            strokeLinecap="round"
          />
        ))
      ) : (
        <>
          <circle
            cx={50}
            cy={50}
            r={34}
            fill={palette.paper}
            stroke={palette.ink}
            strokeWidth={DEFAULT_STAMP_WIDTH}
            strokeDasharray="6 5"
          />
          <text x={50} y={64} textAnchor="middle" fontSize={40} fill={palette.ink}>
            ?
          </text>
        </>
      )}
    </svg>
  );
}
