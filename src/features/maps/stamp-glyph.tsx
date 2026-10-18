import { memo } from "react";
import { Circle, Group, Path, Rect, Text } from "react-konva";
import {
  DEFAULT_STAMP_WIDTH,
  STAMP_BASE_SIZE,
  STAMP_BOX,
  fillColor,
  stampByKey,
  stampPalette,
} from "./stamp-library";

interface GlyphProps {
  icon: string;
  /** 1 = `STAMP_BASE_SIZE` map pixels wide. */
  scale: number;
  /** `#rrggbb` tint, or null for the default ink. */
  color: string | null;
  opacity?: number;
  /** Clickable? The ghost shown by the Stamp tool is not. */
  listening?: boolean;
}

/**
 * The Konva drawing of one stamp, centred on the origin of its parent. Plain
 * `Path` nodes: a few per stamp, which Konva handles fine for hundreds of
 * stamps. Unknown icon keys draw a dashed "?" marker instead of nothing.
 */
export const StampGlyph = memo(function StampGlyph({
  icon,
  scale,
  color,
  opacity = 1,
  listening = true,
}: GlyphProps) {
  const k = (STAMP_BASE_SIZE * scale) / STAMP_BOX;
  const def = stampByKey(icon);
  const palette = stampPalette(color);
  const half = STAMP_BOX / 2;

  return (
    <Group scaleX={k} scaleY={k} offsetX={half} offsetY={half} opacity={opacity}>
      {/* An invisible target, so clicking inside the stamp's body selects it. */}
      {listening && (
        <Rect
          x={12}
          y={12}
          width={76}
          height={76}
          fill="rgba(0,0,0,0)"
          perfectDrawEnabled={false}
        />
      )}
      {def ? (
        def.shapes.map((shape, i) => (
          <Path
            key={i}
            data={shape.d}
            fill={shape.fill === "none" ? undefined : fillColor(shape.fill, palette)}
            stroke={shape.w === 0 ? undefined : palette.ink}
            strokeWidth={shape.w ?? DEFAULT_STAMP_WIDTH}
            lineJoin="round"
            lineCap="round"
            listening={false}
            perfectDrawEnabled={false}
            shadowForStrokeEnabled={false}
          />
        ))
      ) : (
        <>
          <Circle
            x={half}
            y={half}
            radius={34}
            fill={palette.paper}
            stroke={palette.ink}
            strokeWidth={DEFAULT_STAMP_WIDTH}
            dash={[6, 5]}
            listening={false}
          />
          <Text
            x={half - 34}
            y={half - 20}
            width={68}
            align="center"
            text="?"
            fontSize={40}
            fill={palette.ink}
            listening={false}
          />
        </>
      )}
    </Group>
  );
});
