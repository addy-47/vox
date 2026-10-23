import {
  memo,
  useEffect,
  useId,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { cn } from "@/shared/lib/utils";
import { SPEECH_CARD_COPY } from "@/data/settingsCopy";

const H = 112;
const MID = H / 2;
/** Vertical start offsets of the three inlet lanes, converging into the engine. */
const LANES = [-34, 0, 34];
const DUR = 6.5;
/** Hard cap on the outgoing bars. Each bar carries two SMIL animations, so an
 *  uncapped count reached ~130 concurrent timelines on a wide modal. */
const MAX_BARS = 24;

export interface EngineFlowBannerProps {
  /** Engine display name. Shown in the banner shell, not in the drawing. */
  engineName: string;
  words?: readonly string[];
  className?: string;
}

function usePrefersReducedMotion(): boolean {
  const [reduced, setReduced] = useState(
    () => window.matchMedia("(prefers-reduced-motion: reduce)").matches
  );
  useEffect(() => {
    const query = window.matchMedia("(prefers-reduced-motion: reduce)");
    const onChange = (event: MediaQueryListEvent) => setReduced(event.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  }, []);
  return reduced;
}

/**
 * Text flowing through an engine and coming out as audio. A decorative
 * diagram: the shell carries the words, the drawing is `aria-hidden`.
 *
 * Width is measured before paint (`useLayoutEffect`), so the lanes and bars are
 * drawn at their final size on the first frame instead of starting at a default
 * width and jumping. The drawing is clipped to its own box, because the inlet
 * lanes start left of the viewBox.
 */
export const EngineFlowBanner = memo(({
  engineName,
  words = SPEECH_CARD_COPY.engineFlow.words,
  className,
}: EngineFlowBannerProps) => {
  const uid = useId().replace(/:/g, "");
  const wrapRef = useRef<HTMLDivElement | null>(null);
  const svgRef = useRef<SVGSVGElement | null>(null);
  const [width, setWidth] = useState(0);
  const still = usePrefersReducedMotion();

  useLayoutEffect(() => {
    const node = wrapRef.current;
    if (!node) return undefined;
    const initialWidth = Math.round(node.getBoundingClientRect().width);
    setWidth(initialWidth);
    const observer = new ResizeObserver((entries) => {
      const entry = entries[entries.length - 1];
      if (entry) {
        const next = Math.round(entry.contentRect.width);
        setWidth((prev) => (Math.abs(prev - next) >= 2 ? next : prev));
      }
    });
    observer.observe(node);
    return () => observer.disconnect();
  }, []);

  /**
   * SMIL keeps running while the app is hidden or the viewport is being
   * resized. The design-spec §7 animation gate only covers CSS and frame
   * callbacks, so the SVG is paused explicitly for both.
   */
  useEffect(() => {
    const svg = svgRef.current;
    if (!svg) return undefined;
    const sync = () => {
      const resizing = document.documentElement.hasAttribute("data-viewport-resize");
      if (document.hidden || resizing) svg.pauseAnimations();
      else svg.unpauseAnimations();
    };
    sync();
    document.addEventListener("visibilitychange", sync);
    const observer = new MutationObserver(sync);
    observer.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["data-viewport-resize"],
    });
    return () => {
      document.removeEventListener("visibilitychange", sync);
      observer.disconnect();
    };
  }, []);

  const W = Math.max(420, width);
  const cx = W / 2;
  const inlet = cx - 66;
  const outlet = cx + 66;

  const lanes = useMemo(
    () =>
      LANES.map(
        (dy) =>
          `M -20 ${MID + dy} C ${inlet * 0.45} ${MID + dy}, ${inlet * 0.7} ${MID}, ${inlet} ${MID}`
      ),
    [inlet]
  );

  const bars = useMemo(() => {
    const n = Math.min(
      MAX_BARS,
      Math.max(8, Math.floor((W - outlet - 24) / 9))
    );
    return Array.from({ length: n }, (_, i) => {
      const env = Math.exp(-i / (n * 0.7));
      const jitter = 0.55 + 0.45 * Math.abs(Math.sin(i * 1.7) * Math.cos(i * 0.6));
      return {
        x: outlet + 14 + i * 9,
        max: Math.max(8, 74 * env * jitter),
        op: 0.25 + 0.75 * env,
        i,
      };
    });
  }, [W, outlet]);

  // `--accent` is an RGB triplet, so it is only valid inside `rgb()`/`rgba()`.
  const accent = "rgb(var(--accent))";
  const fade = (id: string, x1: number, x2: number, stops: Array<[number, number]>) => (
    <linearGradient id={id} gradientUnits="userSpaceOnUse" x1={x1} x2={x2} y1="0" y2="0">
      {stops.map(([offset, alpha]) => (
        <stop key={offset} offset={offset} stopColor="#fff" stopOpacity={alpha} />
      ))}
    </linearGradient>
  );

  return (
    <div
      className={cn(
        "hidden md:flex shrink-0 items-center gap-5 rounded-2xl border border-[rgba(var(--foreground),0.08)]",
        "bg-[rgba(var(--card),0.75)] px-5 py-3 shadow-lg",
        className
      )}
    >
      <div ref={wrapRef} className="relative min-w-0 flex-1" style={{ height: H }}>
        {width > 0 && (
          <svg
            ref={svgRef}
            width={W}
            height={H}
            viewBox={`0 0 ${W} ${H}`}
            aria-hidden="true"
            focusable="false"
            className="block max-w-full"
            style={{ clipPath: "inset(0)" }}
          >
            <defs>
              {fade(`${uid}l`, 0, inlet, [[0, 0], [0.35, 1]])}
              {fade(`${uid}r`, outlet, W, [[0.5, 1], [1, 0]])}
              <mask id={`${uid}ml`}>
                <rect x="0" y="-20" width={W} height={H + 40} fill={`url(#${uid}l)`} />
              </mask>
              <mask id={`${uid}mr`}>
                <rect x="0" y="-20" width={W} height={H + 40} fill={`url(#${uid}r)`} />
              </mask>
              <radialGradient id={`${uid}g`}>
                <stop offset="0" stopColor={accent} stopOpacity="0.22" />
                <stop offset="1" stopColor={accent} stopOpacity="0" />
              </radialGradient>
            </defs>

            {/* Guide lanes: the path text takes into the engine. */}
            <g
              mask={`url(#${uid}ml)`}
              fill="none"
              stroke="currentColor"
              strokeOpacity="0.1"
              strokeDasharray="2 5"
            >
              {lanes.map((d, i) => (
                <path key={i} d={d} />
              ))}
            </g>

            {/* Words flowing in. */}
            <g
              mask={`url(#${uid}ml)`}
              fill="currentColor"
              fontSize="14"
              fontWeight="500"
              style={{ fontFamily: "inherit" }}
            >
              {words.map((word, i) => {
                const lane = lanes[i % lanes.length];
                const begin = `${-(i * DUR) / words.length}s`;
                return (
                  <text key={`${word}-${i}`} opacity={still ? 0.6 : 0} textAnchor="end" dy="4">
                    {word}
                    {!still && (
                      <>
                        <animateMotion
                          dur={`${DUR}s`}
                          begin={begin}
                          repeatCount="indefinite"
                          rotate="auto"
                          path={lane}
                        />
                        <animate
                          attributeName="opacity"
                          dur={`${DUR}s`}
                          begin={begin}
                          repeatCount="indefinite"
                          values="0;0.8;0.8;0;0"
                          keyTimes="0;0.1;0.68;0.92;1"
                        />
                      </>
                    )}
                  </text>
                );
              })}
            </g>

            {/* The engine: halo, inlet pulse, ticked orb, mini waveform. */}
            <circle cx={cx} cy={MID} r="90" fill={`url(#${uid}g)`} />
            {!still && (
              <circle cx={inlet} cy={MID} r="4" fill="none" stroke={accent} strokeWidth="1.5">
                <animate attributeName="r" values="4;22" dur="1.4s" repeatCount="indefinite" />
                <animate attributeName="opacity" values="0.6;0" dur="1.4s" repeatCount="indefinite" />
              </circle>
            )}
            <g transform={`translate(${cx} ${MID})`}>
              <rect
                x="-66"
                y="-31"
                width="132"
                height="62"
                rx="31"
                fill="color-mix(in srgb, rgb(var(--accent)) 9%, transparent)"
                stroke={accent}
                strokeWidth="1.75"
              />
              <g transform="translate(-30 0)" fill="none" stroke={accent} strokeLinecap="round">
                <g strokeWidth="2" strokeDasharray="2 5.6">
                  <circle r="19" />
                  {!still && (
                    <animateTransform
                      attributeName="transform"
                      type="rotate"
                      from="0"
                      to="360"
                      dur="9s"
                      repeatCount="indefinite"
                    />
                  )}
                </g>
                <g strokeWidth="2">
                  <path d="M -11 0 A 11 11 0 0 1 8 -8" />
                  {!still && (
                    <animateTransform
                      attributeName="transform"
                      type="rotate"
                      from="360"
                      to="0"
                      dur="3.2s"
                      repeatCount="indefinite"
                    />
                  )}
                </g>
                <circle r="4.5" fill={accent} stroke="none">
                  {!still && (
                    <animate attributeName="r" values="3.5;5.5;3.5" dur="1.6s" repeatCount="indefinite" />
                  )}
                </circle>
              </g>
              <g fill={accent}>
                {[0, 1, 2, 3, 4].map((k) => {
                  const hs = [10, 22, 30, 18, 12][k];
                  return (
                    <rect key={k} x={8 + k * 9} y={-hs / 2} width="4" height={hs} rx="2">
                      {!still && (
                        <>
                          <animate
                            attributeName="height"
                            values={`${hs};${hs * 0.3};${hs}`}
                            dur="0.9s"
                            begin={`${-k * 0.17}s`}
                            repeatCount="indefinite"
                          />
                          <animate
                            attributeName="y"
                            values={`${-hs / 2};${-hs * 0.15};${-hs / 2}`}
                            dur="0.9s"
                            begin={`${-k * 0.17}s`}
                            repeatCount="indefinite"
                          />
                        </>
                      )}
                    </rect>
                  );
                })}
              </g>
            </g>

            {/* Audio leaving the engine, fading out to the right. */}
            <g mask={`url(#${uid}mr)`} fill={accent}>
              {bars.map((bar) => {
                const lo = 4;
                const values = [lo, bar.max, lo + bar.max * 0.25, bar.max * 0.7, lo].map((v) =>
                  v.toFixed(1)
                );
                const ys = values.map((v) => (MID - Number(v) / 2).toFixed(1));
                const begin = `${bar.i * 0.075 - 13}s`;
                return (
                  <rect
                    key={bar.i}
                    x={bar.x}
                    width="3.5"
                    rx="1.75"
                    opacity={bar.op}
                    y={MID - (still ? bar.max / 2 : lo / 2)}
                    height={still ? bar.max : lo}
                  >
                    {!still && (
                      <>
                        <animate
                          attributeName="height"
                          values={values.join(";")}
                          dur="1.3s"
                          begin={begin}
                          repeatCount="indefinite"
                        />
                        <animate
                          attributeName="y"
                          values={ys.join(";")}
                          dur="1.3s"
                          begin={begin}
                          repeatCount="indefinite"
                        />
                      </>
                    )}
                  </rect>
                );
              })}
            </g>
          </svg>
        )}
      </div>

      <div className="shrink-0 max-w-[220px] text-right">
        <p className="text-[11px] font-bold uppercase tracking-[0.14em] text-[rgb(var(--accent))]">
          {SPEECH_CARD_COPY.engineFlow.title}
        </p>
        <p className="mt-1 text-[12px] leading-snug text-[rgb(var(--foreground-muted))]">
          {engineName}
        </p>
        <p className="mt-0.5 text-[11px] leading-snug text-[rgb(var(--foreground-muted))]/70">
          {SPEECH_CARD_COPY.engineFlow.caption}
        </p>
      </div>
    </div>
  );
});

EngineFlowBanner.displayName = "EngineFlowBanner";
