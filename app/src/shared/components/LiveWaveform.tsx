import React, { useEffect, useRef, memo, type HTMLAttributes } from "react"
import { cn } from "../lib/utils"
import { useDynamicFPS } from "@/shared/hooks/useDynamicFPS"

export type LiveWaveformProps = HTMLAttributes<HTMLDivElement> & {
  active?: boolean
  processing?: boolean
  deviceId?: string
  barWidth?: number
  barHeight?: number
  barGap?: number
  barRadius?: number
  barColor?: string
  fadeEdges?: boolean
  fadeWidth?: number
  height?: string | number
  sensitivity?: number
  smoothingTimeConstant?: number
  fftSize?: number
  historySize?: number
  updateRate?: number
  mode?: "scrolling" | "static"
  onError?: (error: Error) => void
  onStreamReady?: (stream: MediaStream) => void
  onStreamEnd?: () => void
  telemetryRef?: React.RefObject<any>
}

export const LiveWaveform = memo(({
  active = false,
  processing = false,
  deviceId,
  barWidth = 3,
  barGap = 1,
  barRadius = 1.5,
  barColor,
  fadeEdges = true,
  fadeWidth = 24,
  barHeight: baseBarHeight = 4,
  height = 64,
  sensitivity = 1,
  smoothingTimeConstant = 0.8,
  fftSize = 256,
  historySize = 60,
  updateRate = 30,
  mode = "static",
  onError,
  onStreamReady,
  onStreamEnd,
  className,
  telemetryRef,
  ...props
}: LiveWaveformProps) => {
  const canvasRef = useRef<HTMLCanvasElement>(null)
  const containerRef = useRef<HTMLDivElement>(null)
  const historyRef = useRef<number[]>([])
  const analyserRef = useRef<AnalyserNode | null>(null)
  const audioContextRef = useRef<AudioContext | null>(null)
  const streamRef = useRef<MediaStream | null>(null)
  const lastUpdateRef = useRef<number>(0)
  const processingAnimationRef = useRef<number | null>(null)
  const lastActiveDataRef = useRef<number[]>([])
  const transitionProgressRef = useRef(0)
  const staticBarsRef = useRef<number[]>([])
  const needsRedrawRef = useRef(true)
  const gradientCacheRef = useRef<CanvasGradient | null>(null)
  const barGradientCacheRef = useRef<CanvasGradient | null>(null)
  const lastWidthRef = useRef(0)
  const lastHeightRef = useRef(0)

  const heightStyle = typeof height === "number" ? `${height}px` : height

  // ── Dynamic FPS: tick function stored in ref ──
  const tickRef = useRef<(dt: number) => void>(() => {})

  useDynamicFPS({
    onFrame: (dt) => tickRef.current(dt),
    isActive: active || processing,
    isPaused: !active && !processing,
    fpsActive: 60,
    fpsIdle: 0,
  })

  // Handle canvas resizing
  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const resizeObserver = new ResizeObserver(() => {
      const rect = container.getBoundingClientRect()
      if (rect.width === 0 || rect.height === 0) return

      const dpr = window.devicePixelRatio || 1
      canvas.width = rect.width * dpr
      canvas.height = rect.height * dpr
      canvas.style.width = `${rect.width}px`
      canvas.style.height = `${rect.height}px`

      const ctx = canvas.getContext("2d", { alpha: true })
      if (ctx) {
        ctx.scale(dpr, dpr)
      }

      gradientCacheRef.current = null
      barGradientCacheRef.current = null
      lastWidthRef.current = rect.width
      lastHeightRef.current = rect.height
      needsRedrawRef.current = true
    })

    resizeObserver.observe(container)
    return () => resizeObserver.disconnect()
  }, [])

  // Processing & Idle transitions
  useEffect(() => {
    if (processing && !active) {
      let time = 0
      transitionProgressRef.current = 0

      const animateProcessing = () => {
        time += 0.05 // Slightly faster for more energy
        transitionProgressRef.current = Math.min(
          1,
          transitionProgressRef.current + 0.05
        )

        const containerWidth = containerRef.current?.getBoundingClientRect().width || 200
        const step = barWidth + barGap
        const barCount = Math.floor(containerWidth / step)
        const processingData = new Array(barCount)

        if (mode === "static") {
          const halfCount = Math.floor(barCount / 2)
          for (let i = 0; i < barCount; i++) {
            const normalizedPosition = (i - halfCount) / halfCount
            const centerWeight = 1 - Math.abs(normalizedPosition) * 0.4
            const wave1 = Math.sin(time * 3.0 + normalizedPosition * 3) * 0.4
            const wave2 = Math.sin(time * 1.8 - normalizedPosition * 5) * 0.25
            const processingValue = (0.4 + wave1 + wave2) * centerWeight
            
            let finalValue = processingValue
            if (lastActiveDataRef.current.length > 0 && transitionProgressRef.current < 1) {
              const lastDataIndex = Math.min(i, lastActiveDataRef.current.length - 1)
              const lastValue = lastActiveDataRef.current[lastDataIndex] || 0
              finalValue = lastValue * (1 - transitionProgressRef.current) + processingValue * transitionProgressRef.current
            }
            processingData[i] = Math.max(0.1, Math.min(1, finalValue))
          }
          staticBarsRef.current = processingData
        } else {
          for (let i = 0; i < barCount; i++) {
            const wave1 = Math.sin(time * 3.0 + i * 0.15) * 0.35
            const wave2 = Math.sin(time * 1.5 - i * 0.08) * 0.2
            const processingValue = (0.35 + wave1 + wave2)
            
            let finalValue = processingValue
            if (lastActiveDataRef.current.length > 0 && transitionProgressRef.current < 1) {
              const lastDataIndex = Math.floor((i / barCount) * lastActiveDataRef.current.length)
              const lastValue = lastActiveDataRef.current[lastDataIndex] || 0
              finalValue = lastValue * (1 - transitionProgressRef.current) + processingValue * transitionProgressRef.current
            }
            processingData[i] = Math.max(0.1, Math.min(1, finalValue))
          }
          historyRef.current = processingData
        }

        needsRedrawRef.current = true
        processingAnimationRef.current = requestAnimationFrame(animateProcessing)
      }

      animateProcessing()
      return () => {
        if (processingAnimationRef.current) cancelAnimationFrame(processingAnimationRef.current)
      }
    } else if (!active && !processing) {
      // Idle fade down
      const fadeToIdle = () => {
        let stillFading = false
        if (mode === "static") {
          staticBarsRef.current = staticBarsRef.current.map(v => {
            if (v > 0.06) {
              stillFading = true
              return v * 0.92
            }
            return 0.05
          })
        } else {
          historyRef.current = historyRef.current.map(v => {
            if (v > 0.06) {
              stillFading = true
              return v * 0.92
            }
            return 0.05
          })
        }
        
        if (stillFading) {
          needsRedrawRef.current = true
          processingAnimationRef.current = requestAnimationFrame(fadeToIdle)
        } else {
          needsRedrawRef.current = true
        }
      }
      fadeToIdle()
      return () => {
        if (processingAnimationRef.current) cancelAnimationFrame(processingAnimationRef.current)
      }
    }
    return undefined
  }, [processing, active, barWidth, barGap, mode])

  // Mic setup (only if no telemetryRef provided)
  useEffect(() => {
    if (!active || telemetryRef?.current) return

    const setupMicrophone = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: deviceId ? { deviceId: { exact: deviceId } } : true,
        })
        streamRef.current = stream
        onStreamReady?.(stream)

        const AudioContextConstructor = window.AudioContext || (window as any).webkitAudioContext
        const audioContext = new AudioContextConstructor()
        const analyser = audioContext.createAnalyser()
        analyser.fftSize = fftSize
        analyser.smoothingTimeConstant = smoothingTimeConstant
        const source = audioContext.createMediaStreamSource(stream)
        source.connect(analyser)

        audioContextRef.current = audioContext
        analyserRef.current = analyser
      } catch (error) {
        onError?.(error as Error)
      }
    }

    setupMicrophone()
    return () => {
      streamRef.current?.getTracks().forEach(t => t.stop())
      audioContextRef.current?.close()
      analyserRef.current = null
    }
  }, [active, deviceId, fftSize, smoothingTimeConstant, telemetryRef])

  // Animation & Rendering Loop
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return
    const ctx = canvas.getContext("2d", { alpha: true })
    if (!ctx) return

    tickRef.current = (_dt: number) => {
      const currentTime = performance.now()
      const width = lastWidthRef.current
      const height = lastHeightRef.current
      if (width === 0) return

      // 1. Data Update (Throttle to updateRate)
      if (active && (currentTime - lastUpdateRef.current > updateRate)) {
        lastUpdateRef.current = currentTime
        const externalData = telemetryRef?.current

        if (mode === "static") {
          const step = barWidth + barGap
          const barCount = Math.floor(width / step)
          const halfCount = Math.floor(barCount / 2)
          const newBars = new Array(barCount)
          
          let relevantData: number[] = []
          if (externalData) {
            const energy = typeof externalData === 'number' ? externalData : (externalData.energy || 0)
            for (let i = 0; i < halfCount; i++) {
              const variation = (0.7 + Math.sin(i * 0.8 + currentTime * 0.005) * 0.3) * (0.9 + Math.random() * 0.1)
              relevantData.push(energy * variation)
            }
          } else if (analyserRef.current) {
            const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount)
            analyserRef.current.getByteFrequencyData(dataArray)
            relevantData = Array.from(dataArray.slice(10, 60)).map(v => v / 255)
          }

          for (let i = 0; i < halfCount; i++) {
            const dataIndex = Math.floor((i / halfCount) * relevantData.length)
            const value = Math.min(1, Math.max(0.1, (relevantData[dataIndex] || 0) * sensitivity))
            newBars[halfCount - 1 - i] = value
            newBars[halfCount + i] = value
          }
          staticBarsRef.current = newBars
          lastActiveDataRef.current = newBars
        } else {
          // Scrolling Mode
          let energy = 0
          if (externalData) {
            energy = typeof externalData === 'number' ? externalData : (externalData.energy || 0)
          } else if (analyserRef.current) {
            const dataArray = new Uint8Array(analyserRef.current.frequencyBinCount)
            analyserRef.current.getByteFrequencyData(dataArray)
            energy = dataArray.slice(5, 50).reduce((a, b) => a + b, 0) / (45 * 255)
          }
          
          historyRef.current.push(Math.min(1, Math.max(0.1, energy * sensitivity)))
          if (historyRef.current.length > historySize) historyRef.current.shift()
          lastActiveDataRef.current = historyRef.current
        }
        needsRedrawRef.current = true
      }

      // 2. Redraw (Only if needed or active)
      if (needsRedrawRef.current || active || processing) {
        ctx.clearRect(0, 0, width, height)
        
        const step = barWidth + barGap
        const barCount = Math.floor(width / step)
        const centerY = height / 2
        
        const totalContentWidth = barCount * step - barGap
        const startX = (width - totalContentWidth) / 2

        if (!barGradientCacheRef.current || lastHeightRef.current !== height) {
          const accentVal = getComputedStyle(document.documentElement).getPropertyValue('--accent').trim() || "0, 219, 233";
          const g = ctx.createLinearGradient(0, 0, 0, height)
          g.addColorStop(0, `rgba(${accentVal}, 0.05)`)
          g.addColorStop(0.3, `rgba(${accentVal}, 0.6)`)
          g.addColorStop(0.5, `rgb(${accentVal})`) 
          g.addColorStop(0.7, `rgba(${accentVal}, 0.6)`)
          g.addColorStop(1, `rgba(${accentVal}, 0.05)`)
          barGradientCacheRef.current = g
        }
        
        const fillColor = barColor || barGradientCacheRef.current
        const data = mode === "static" ? staticBarsRef.current : historyRef.current
        
        ctx.fillStyle = fillColor

        if (mode === "static") {
          for (let i = 0; i < barCount; i++) {
            const val = data[i] || 0.1
            const bH = Math.max(baseBarHeight, val * height * 0.9)
            const x = startX + i * step
            const y = centerY - bH / 2
            
            ctx.globalAlpha = 0.4 + val * 0.6
            if (barRadius > 0 && barWidth > 2) {
              ctx.beginPath()
              ctx.roundRect(x, y, barWidth, bH, barRadius)
              ctx.fill()
            } else {
              ctx.fillRect(x, y, barWidth, bH)
            }
          }
        } else {
          const actualData = data.length > 0 ? data : new Array(barCount).fill(0.1)
          for (let i = 0; i < barCount; i++) {
            const dataIdx = actualData.length - 1 - i
            if (dataIdx < 0) break
            
            const val = actualData[dataIdx] || 0.1
            const bH = Math.max(baseBarHeight, val * height * 0.9)
            const x = startX + totalContentWidth - (i + 1) * step + barGap
            const y = centerY - bH / 2
            
            ctx.globalAlpha = 0.4 + val * 0.6
            ctx.fillRect(x, y, barWidth, bH)
          }
        }

        if (fadeEdges && fadeWidth > 0) {
          if (!gradientCacheRef.current || lastWidthRef.current !== width) {
            const g = ctx.createLinearGradient(0, 0, width, 0)
            const stop = Math.min(0.45, fadeWidth / width)
            g.addColorStop(0, "rgba(0,0,0,1)")
            g.addColorStop(stop, "rgba(0,0,0,0)")
            g.addColorStop(1 - stop, "rgba(0,0,0,0)")
            g.addColorStop(1, "rgba(0,0,0,1)")
            gradientCacheRef.current = g
          }
          ctx.globalCompositeOperation = "destination-out"
          ctx.fillStyle = gradientCacheRef.current
          ctx.fillRect(0, 0, width, height)
          ctx.globalCompositeOperation = "source-over"
        }
        
        if (!active && !processing) needsRedrawRef.current = false
      }
    }

    return () => {
      tickRef.current = () => {}
    }
  }, [active, processing, sensitivity, updateRate, historySize, barWidth, baseBarHeight, barGap, barRadius, barColor, fadeEdges, fadeWidth, mode])

  return (
    <div
      className={cn("relative h-full w-full overflow-hidden", className)}
      ref={containerRef}
      style={{ height: heightStyle }}
      {...props}
    >
      <canvas className="block h-full w-full" ref={canvasRef} />
    </div>
  )
})

LiveWaveform.displayName = "LiveWaveform"
