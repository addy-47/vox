import { useEffect, useRef, type HTMLAttributes } from "react"

import { cn } from "../lib/utils"

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

export const LiveWaveform = ({
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
  const animationRef = useRef<number>(0)
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
  const lastColorRef = useRef<string | null>(null)

  const heightStyle = typeof height === "number" ? `${height}px` : height

  // Handle canvas resizing
  useEffect(() => {
    const canvas = canvasRef.current
    const container = containerRef.current
    if (!canvas || !container) return

    const resizeObserver = new ResizeObserver(() => {
      const rect = container.getBoundingClientRect()
      const dpr = window.devicePixelRatio || 1

      canvas.width = rect.width * dpr
      canvas.height = rect.height * dpr
      canvas.style.width = `${rect.width}px`
      canvas.style.height = `${rect.height}px`

      const ctx = canvas.getContext("2d")
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

  useEffect(() => {
    if (processing && !active) {
      let time = 0
      transitionProgressRef.current = 0

      const animateProcessing = () => {
        time += 0.03
        transitionProgressRef.current = Math.min(
          1,
          transitionProgressRef.current + 0.02
        )

        const processingData = []
        const barCount = Math.floor(
          (containerRef.current?.getBoundingClientRect().width || 200) /
            (barWidth + barGap)
        )

        if (mode === "static") {
          const halfCount = Math.floor(barCount / 2)

          for (let i = 0; i < barCount; i++) {
            const normalizedPosition = (i - halfCount) / halfCount
            const centerWeight = 1 - Math.abs(normalizedPosition) * 0.5

            // Synchronized fluid waves
            const wave1 = Math.sin(time * 2.0 + normalizedPosition * 2) * 0.3
            const wave2 = Math.sin(time * 1.2 - normalizedPosition * 4) * 0.2
            const processingValue = (0.3 + wave1 + wave2) * centerWeight

            let finalValue = processingValue
            if (
              lastActiveDataRef.current.length > 0 &&
              transitionProgressRef.current < 1
            ) {
              const lastDataIndex = Math.min(
                i,
                lastActiveDataRef.current.length - 1
              )
              const lastValue = lastActiveDataRef.current[lastDataIndex] || 0
              finalValue =
                lastValue * (1 - transitionProgressRef.current) +
                processingValue * transitionProgressRef.current
            }

            processingData[i] = Math.max(0.05, Math.min(1, finalValue))
          }
        } else {
          for (let i = 0; i < barCount; i++) {
            const wave1 = Math.sin(time * 2.0 + i * 0.1) * 0.25
            const wave2 = Math.sin(time * 0.8 - i * 0.05) * 0.15
            const processingValue = (0.25 + wave1 + wave2)

            let finalValue = processingValue
            if (
              lastActiveDataRef.current.length > 0 &&
              transitionProgressRef.current < 1
            ) {
              const lastDataIndex = Math.floor(
                (i / barCount) * lastActiveDataRef.current.length
              )
              const lastValue = lastActiveDataRef.current[lastDataIndex] || 0
              finalValue =
                lastValue * (1 - transitionProgressRef.current) +
                processingValue * transitionProgressRef.current
            }

            processingData[i] = Math.max(0.05, Math.min(1, finalValue))
          }
        }

        if (mode === "static") {
          staticBarsRef.current = processingData
        } else {
          historyRef.current = processingData
        }

        needsRedrawRef.current = true
        processingAnimationRef.current =
          requestAnimationFrame(animateProcessing)
      }

      animateProcessing()

      return () => {
        if (processingAnimationRef.current) {
          cancelAnimationFrame(processingAnimationRef.current)
        }
      }
    } else if (!active && !processing) {
      const hasData =
        mode === "static"
          ? staticBarsRef.current.length > 0
          : historyRef.current.length > 0

      if (hasData) {
        let fadeProgress = 0
        const fadeToIdle = () => {
          fadeProgress += 0.03
          if (fadeProgress < 1) {
            if (mode === "static") {
              staticBarsRef.current = staticBarsRef.current.map(
                (value) => value * (1 - fadeProgress)
              )
            } else {
              historyRef.current = historyRef.current.map(
                (value) => value * (1 - fadeProgress)
              )
            }
            needsRedrawRef.current = true
            requestAnimationFrame(fadeToIdle)
          } else {
            if (mode === "static") {
              staticBarsRef.current = []
            } else {
              historyRef.current = []
            }
          }
        }
        fadeToIdle()
      }
    }
    return undefined;
  }, [processing, active, barWidth, barGap, mode])

  // Handle microphone setup and teardown
  useEffect(() => {
    if (!active) {
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop())
        streamRef.current = null
        onStreamEnd?.()
      }
      if (
        audioContextRef.current &&
        audioContextRef.current.state !== "closed"
      ) {
        audioContextRef.current.close()
        audioContextRef.current = null
      }
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
        animationRef.current = 0
      }
      return
    }

    const setupMicrophone = async () => {
      try {
        const stream = await navigator.mediaDevices.getUserMedia({
          audio: deviceId
            ? {
                deviceId: { exact: deviceId },
                echoCancellation: true,
                noiseSuppression: true,
                autoGainControl: true,
              }
            : {
                echoCancellation: true,
                noiseSuppression: true,
                autoGainControl: true,
              },
        })
        streamRef.current = stream
        onStreamReady?.(stream)

        const AudioContextConstructor =
          window.AudioContext ||
          (window as unknown as { webkitAudioContext: typeof AudioContext })
            .webkitAudioContext
        const audioContext = new AudioContextConstructor()
        const analyser = audioContext.createAnalyser()
        analyser.fftSize = fftSize
        analyser.smoothingTimeConstant = smoothingTimeConstant

        const source = audioContext.createMediaStreamSource(stream)
        source.connect(analyser)

        audioContextRef.current = audioContext
        analyserRef.current = analyser

        // Clear history when starting
        historyRef.current = []
      } catch (error) {
        onError?.(error as Error)
      }
    }

    setupMicrophone()

    return () => {
      if (streamRef.current) {
        streamRef.current.getTracks().forEach((track) => track.stop())
        streamRef.current = null
        onStreamEnd?.()
      }
      if (
        audioContextRef.current &&
        audioContextRef.current.state !== "closed"
      ) {
        audioContextRef.current.close()
        audioContextRef.current = null
      }
      if (animationRef.current) {
        cancelAnimationFrame(animationRef.current)
        animationRef.current = 0
      }
    }
  }, [
    active,
    deviceId,
    fftSize,
    smoothingTimeConstant,
    onError,
    onStreamReady,
    onStreamEnd,
  ])

  // Animation loop
  useEffect(() => {
    const canvas = canvasRef.current
    if (!canvas) return

    const ctx = canvas.getContext("2d")
    if (!ctx) return

    let rafId: number

    const animate = (currentTime: number) => {
      // Render waveform
      const rect = canvas.getBoundingClientRect()

      // Update audio data if active
      if (active && currentTime - lastUpdateRef.current > updateRate) {
        lastUpdateRef.current = currentTime

        if (analyserRef.current) {
          const dataArray = new Uint8Array(
            analyserRef.current.frequencyBinCount
          )
          analyserRef.current.getByteFrequencyData(dataArray)
        }

        const externalData = telemetryRef?.current

        if (analyserRef.current || externalData) {
          if (mode === "static") {
            // For static mode, update bars in place
            let relevantData: number[] = []
            if (externalData) {
              if (Array.isArray(externalData)) {
                relevantData = externalData
              } else {
                const energy = typeof externalData === 'number' ? externalData : (externalData.energy || 0)
                // Spread single energy value across bars with some symmetric variation
                const barCount = Math.floor(rect.width / (barWidth + barGap))
                const halfCount = Math.floor(barCount / 2)
                for (let i = 0; i < halfCount; i++) {
                  // Merge 'amps' by using a lower-frequency sine and more synchronized variation
                  const groupIndex = Math.floor(i / 4) // Synchronize every 4 bars
                  const variation = (0.7 + Math.sin(groupIndex * 0.8 + currentTime * 0.002) * 0.2) * 
                                  (0.9 + Math.random() * 0.1)
                  relevantData.push(energy * variation)
                }
              }
            } else if (analyserRef.current) {
              const dataArray = new Uint8Array(
                analyserRef.current.frequencyBinCount
              )
              analyserRef.current.getByteFrequencyData(dataArray)
              const startFreq = Math.floor(dataArray.length * 0.05)
              const endFreq = Math.floor(dataArray.length * 0.4)
              const slicedData = dataArray.slice(startFreq, endFreq)
              relevantData = Array.from(slicedData).map(v => v / 255)
            }

            const barCount = Math.floor(rect.width / (barWidth + barGap))
            const halfCount = Math.floor(barCount / 2)
            const newBars: number[] = new Array(barCount)

            // Symmetric synchronized spreading
            for (let i = 0; i < halfCount; i++) {
              const dataIndex = Math.floor((i / halfCount) * relevantData.length)
              const baseValue = relevantData[dataIndex] || 0
              
              // Coherent variation across groups of bars
              const groupIndex = Math.floor(i / 3) 
              const variation = (0.8 + Math.sin(groupIndex * 0.5 + currentTime * 0.003) * 0.2)
              const value = Math.min(1, baseValue * variation * sensitivity)
              const finalVal = Math.max(0.05, value)
              
              // Mirror to both sides
              newBars[halfCount - 1 - i] = finalVal
              newBars[halfCount + i] = finalVal
            }

            staticBarsRef.current = newBars
            lastActiveDataRef.current = newBars
          } else {
            // Scrolling mode - original behavior
            let average = 0
            if (externalData) {
              average = typeof externalData === 'number' ? externalData : (externalData.energy || 0)
              average *= sensitivity
            } else if (analyserRef.current) {
              const dataArray = new Uint8Array(
                analyserRef.current.frequencyBinCount
              )
              analyserRef.current.getByteFrequencyData(dataArray)
              let sum = 0
              const startFreq = Math.floor(dataArray.length * 0.05)
              const endFreq = Math.floor(dataArray.length * 0.4)
              const relevantData = dataArray.slice(startFreq, endFreq)

              for (let i = 0; i < relevantData.length; i++) {
                sum += relevantData[i]
              }
              average = (sum / relevantData.length / 255) * sensitivity
            }

            // Add to history
            historyRef.current.push(Math.min(1, Math.max(0.05, average)))
            
            // Maintain history size
            if (historyRef.current.length > historySize) {
              historyRef.current.shift()
            }
            
            lastActiveDataRef.current = historyRef.current
          }
          needsRedrawRef.current = true
        }
      }

      // Only redraw if needed
      if (!needsRedrawRef.current && !active) {
        rafId = requestAnimationFrame(animate)
        return
      }

      needsRedrawRef.current = active
      ctx.clearRect(0, 0, rect.width, rect.height)

      const step = barWidth + barGap
      const barCount = Math.floor(rect.width / step)
      const centerY = rect.height / 2

      const computedBarColor =
        barColor ||
        (() => {
          if (barGradientCacheRef.current && lastHeightRef.current === rect.height) {
            return barGradientCacheRef.current
          }

          // Create a premium cyan-blue gradient with better depth
          const gradient = ctx.createLinearGradient(0, centerY - rect.height/2, 0, centerY + rect.height/2)
          gradient.addColorStop(0, "rgba(0, 242, 255, 0.2)") // Top fade
          gradient.addColorStop(0.3, "rgba(0, 242, 255, 0.9)") // Bright cyan
          gradient.addColorStop(0.5, "#00d2ff") // Sky blue core
          gradient.addColorStop(0.7, "rgba(0, 242, 255, 0.9)") // Bright cyan
          gradient.addColorStop(1, "rgba(0, 242, 255, 0.2)") // Bottom fade
          
          barGradientCacheRef.current = gradient
          lastHeightRef.current = rect.height
          lastColorRef.current = "cyan-gradient"
          return gradient
        })()

      // Draw bars based on mode
      if (mode === "static") {
        // Static mode - bars in fixed positions
        const dataToRender = processing
          ? staticBarsRef.current
          : active
            ? staticBarsRef.current
            : staticBarsRef.current.length > 0
              ? staticBarsRef.current
              : []

        for (let i = 0; i < barCount && i < dataToRender.length; i++) {
          const value = dataToRender[i] || 0.1
          const x = i * step
          const barHeight = Math.max(baseBarHeight, value * rect.height * 0.8)
          const y = centerY - barHeight / 2

          ctx.fillStyle = computedBarColor
          ctx.globalAlpha = 0.4 + value * 0.6

          if (barRadius > 0) {
            ctx.beginPath()
            ctx.roundRect(x, y, barWidth, barHeight, barRadius)
            ctx.fill()
          } else {
            ctx.fillRect(x, y, barWidth, barHeight)
          }
        }
      } else {
        // Scrolling mode - original behavior
        for (let i = 0; i < barCount && i < historyRef.current.length; i++) {
          const dataIndex = historyRef.current.length - 1 - i
          const value = historyRef.current[dataIndex] || 0.1
          
          // Fix: Fill the entire width even if history is short, but scroll from right
          // To make it look "centered" or "scrolling through", we use historySize
          const x = rect.width - (i + 1) * step
          
          const barHeight = Math.max(baseBarHeight, value * rect.height * 0.8)
          const y = centerY - barHeight / 2

          ctx.fillStyle = computedBarColor
          ctx.globalAlpha = 0.4 + value * 0.6

          if (barRadius > 0) {
            ctx.fillRect(x, y, barWidth, barHeight) // Optimized for speed, roundRect is expensive
          } else {
            ctx.fillRect(x, y, barWidth, barHeight)
          }
        }
      }

      // Apply edge fading
      if (fadeEdges && fadeWidth > 0 && rect.width > 0) {
        // Cache gradient if width hasn't changed
        if (!gradientCacheRef.current || lastWidthRef.current !== rect.width) {
          const gradient = ctx.createLinearGradient(0, 0, rect.width, 0)
          const fadePercent = Math.min(0.3, fadeWidth / rect.width)

          // destination-out: removes destination where source alpha is high
          // We want: fade edges out, keep center solid
          // Left edge: start opaque (1) = remove, fade to transparent (0) = keep
          gradient.addColorStop(0, "rgba(255,255,255,1)")
          gradient.addColorStop(fadePercent, "rgba(255,255,255,0)")
          // Center stays transparent = keep everything
          gradient.addColorStop(1 - fadePercent, "rgba(255,255,255,0)")
          // Right edge: fade from transparent (0) = keep to opaque (1) = remove
          gradient.addColorStop(1, "rgba(255,255,255,1)")

          gradientCacheRef.current = gradient
          lastWidthRef.current = rect.width
        }

        ctx.globalCompositeOperation = "destination-out"
        ctx.fillStyle = gradientCacheRef.current
        ctx.fillRect(0, 0, rect.width, rect.height)
        ctx.globalCompositeOperation = "source-over"
      }

      ctx.globalAlpha = 1

      rafId = requestAnimationFrame(animate)
    }

    rafId = requestAnimationFrame(animate)

    return () => {
      if (rafId) {
        cancelAnimationFrame(rafId)
      }
    }
  }, [
    active,
    processing,
    sensitivity,
    updateRate,
    historySize,
    barWidth,
    baseBarHeight,
    barGap,
    barRadius,
    barColor,
    fadeEdges,
    fadeWidth,
    mode,
  ])

  return (
    <div
      className={cn("relative h-full w-full", className)}
      ref={containerRef}
      style={{ height: heightStyle }}
      aria-label={
        active
          ? "Live audio waveform"
          : processing
            ? "Processing audio"
            : "Audio waveform idle"
      }
      role="img"
      {...props}
    >
      {!active && !processing && (
        <div className="border-muted-foreground/20 absolute top-1/2 right-0 left-0 -translate-y-1/2 border-t-2 border-dotted" />
      )}
      <canvas
        className="block h-full w-full"
        ref={canvasRef}
        aria-hidden="true"
      />
    </div>
  )
}
