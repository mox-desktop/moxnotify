"use client"

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from "recharts"
import type { DbusNotification } from "@/lib/types"
import type { TimeRange, AbsoluteTimeRange } from "./time-range-picker"
import { useMemo, useState, useCallback, useRef, useEffect } from "react"

interface NotificationChartProps {
  notifications: DbusNotification[]
  timeInterval: number // in minutes
  timeRange?: TimeRange
  onTimeRangeSelect?: (range: AbsoluteTimeRange) => void
}

export function NotificationChart({ notifications, timeInterval, timeRange, onTimeRangeSelect }: NotificationChartProps) {
  const [isSelecting, setIsSelecting] = useState(false)
  const [selectionStart, setSelectionStart] = useState<number | null>(null)
  const [selectionEnd, setSelectionEnd] = useState<number | null>(null)
  const chartContainerRef = useRef<HTMLDivElement>(null)
  const chartData = useMemo(() => {
    const groupedByTime = new Map<number, { Low: number; Normal: number; Critical: number; timestamp: Date }>()

    // First, group notifications by time interval
    notifications.forEach((notification) => {
      // Ensure timestamp is a Date object
      const timestamp = notification.timestamp instanceof Date 
        ? notification.timestamp 
        : new Date(notification.timestamp)
      
      // Round timestamp down to nearest interval boundary
      const time = timestamp.getTime()
      if (isNaN(time)) return // Skip invalid timestamps
      
      const intervalMs = timeInterval * 60 * 1000
      const roundedTime = Math.floor(time / intervalMs) * intervalMs

      if (!groupedByTime.has(roundedTime)) {
        groupedByTime.set(roundedTime, {
          Low: 0,
          Normal: 0,
          Critical: 0,
          timestamp: new Date(roundedTime),
        })
      }

      const counts = groupedByTime.get(roundedTime)!
      // Handle invalid urgency values - default to Normal (1)
      const urgency = notification.urgency === 0 || notification.urgency === 1 || notification.urgency === 2
        ? notification.urgency
        : 1
      
      switch (urgency) {
        case 0:
          counts.Low++
          break
        case 1:
          counts.Normal++
          break
        case 2:
          counts.Critical++
          break
      }
    })

    const intervalMs = timeInterval * 60 * 1000
    const now = Date.now()
    
    // Determine the time range to display
    let startTime: number
    let endTime: number
    
    if (!timeRange || (timeRange.type === "relative" && timeRange.value === "all")) {
      // Use the range of actual notifications
      if (notifications.length === 0) {
        return []
      }

      const allTimes = notifications
        .map((n) => {
          const timestamp = n.timestamp instanceof Date ? n.timestamp : new Date(n.timestamp)
          return timestamp.getTime()
        })
        .filter((t) => !isNaN(t))

      if (allTimes.length === 0) {
        return []
      }

      const minTime = Math.min(...allTimes)
      const maxTime = Math.max(...allTimes)
      
      // Round to interval boundaries
      startTime = Math.floor(minTime / intervalMs) * intervalMs
      endTime = Math.ceil(maxTime / intervalMs) * intervalMs
    } else if (timeRange.type === "relative") {
      // Use the specified relative time range (show last N minutes)
      // Round endTime down to match how notifications are bucketed (rounded down)
      endTime = Math.floor(now / intervalMs) * intervalMs
      if (timeRange.value !== "all" && typeof timeRange.value === "number") {
        startTime = endTime - (timeRange.value * 60 * 1000)
        startTime = Math.floor(startTime / intervalMs) * intervalMs
      } else {
        // "all" case is handled above
        startTime = endTime
      }
    } else {
      // Absolute time range
      startTime = Math.floor(timeRange.from.getTime() / intervalMs) * intervalMs
      endTime = Math.ceil(timeRange.to.getTime() / intervalMs) * intervalMs
    }

    // Create buckets for the entire range, including empty ones
    const result: Array<{ time: string; timestamp: number; Low: number; Normal: number; Critical: number }> = []
    
    for (let time = startTime; time <= endTime; time += intervalMs) {
      const bucket = groupedByTime.get(time)
      result.push({
        time: new Date(time).toLocaleTimeString("en-US", {
          hour: "2-digit",
          minute: "2-digit",
          hour12: false,
        }),
        timestamp: time,
        Low: bucket?.Low || 0,
        Normal: bucket?.Normal || 0,
        Critical: bucket?.Critical || 0,
      })
    }

    return result.sort((a, b) => a.timestamp - b.timestamp)
  }, [notifications, timeInterval, timeRange])

  // Convert X coordinate to timestamp using the chart's coordinate system
  const getTimestampFromX = useCallback((clientX: number): number | null => {
    if (!chartData || chartData.length === 0 || !chartContainerRef.current) {
      return null
    }

    const container = chartContainerRef.current
    // Find the SVG element which contains the actual chart
    const svgElement = container.querySelector('svg')
    if (!svgElement) return null

    const svgRect = svgElement.getBoundingClientRect()
    const relativeX = clientX - svgRect.left
    
    // Find the chart content area (excluding margins)
    // The XAxis is typically at the bottom, so we can use it as reference
    const xAxisGroup = svgElement.querySelector('.recharts-cartesian-axis')
    if (!xAxisGroup) return null
    
    const xAxisRect = xAxisGroup.getBoundingClientRect()
    const plotLeft = xAxisRect.left - svgRect.left
    const plotWidth = xAxisRect.width
    
    const plotRelativeX = relativeX - plotLeft
    
    if (plotRelativeX < 0 || plotRelativeX > plotWidth) {
      return null
    }

    // Find the data point that corresponds to this X position
    const dataPointWidth = plotWidth / chartData.length
    const dataIndex = Math.floor(plotRelativeX / dataPointWidth)
    const clampedIndex = Math.max(0, Math.min(dataIndex, chartData.length - 1))
    
    return chartData[clampedIndex].timestamp
  }, [chartData])

  const handleMouseDown = useCallback((e: React.MouseEvent) => {
    if (!onTimeRangeSelect) return
    
    const timestamp = getTimestampFromX(e.clientX)
    if (timestamp !== null) {
      setIsSelecting(true)
      setSelectionStart(timestamp)
      setSelectionEnd(timestamp)
      e.preventDefault()
    }
  }, [getTimestampFromX, onTimeRangeSelect])

  // Global mouse move handler
  useEffect(() => {
    if (!isSelecting) return

    const handleGlobalMouseMove = (e: MouseEvent) => {
      const timestamp = getTimestampFromX(e.clientX)
      if (timestamp !== null && selectionStart !== null) {
        setSelectionEnd(timestamp)
      }
    }

    const handleGlobalMouseUp = () => {
      if (!isSelecting || selectionStart === null || selectionEnd === null || !onTimeRangeSelect) {
        setIsSelecting(false)
        setSelectionStart(null)
        setSelectionEnd(null)
        return
      }

      const from = Math.min(selectionStart, selectionEnd)
      const to = Math.max(selectionStart, selectionEnd)

      if (from !== to) {
        const intervalMs = timeInterval * 60 * 1000
        const fromDate = new Date(from)
        const toDate = new Date(to + intervalMs - 1)
        
        const absoluteRange: AbsoluteTimeRange = { type: "absolute", from: fromDate, to: toDate }
        onTimeRangeSelect(absoluteRange)
      }

      setIsSelecting(false)
      setSelectionStart(null)
      setSelectionEnd(null)
    }

    window.addEventListener('mousemove', handleGlobalMouseMove)
    window.addEventListener('mouseup', handleGlobalMouseUp)

    return () => {
      window.removeEventListener('mousemove', handleGlobalMouseMove)
      window.removeEventListener('mouseup', handleGlobalMouseUp)
    }
  }, [isSelecting, selectionStart, selectionEnd, onTimeRangeSelect, timeInterval, getTimestampFromX])

  // Calculate selection overlay position
  const selectionOverlay = useMemo(() => {
    if (!isSelecting || selectionStart === null || selectionEnd === null || !chartData || chartData.length === 0 || !chartContainerRef.current) {
      return null
    }

    const container = chartContainerRef.current
    const svgElement = container.querySelector('svg')
    if (!svgElement) return null

    const xAxisGroup = svgElement.querySelector('.recharts-cartesian-axis')
    if (!xAxisGroup) return null
    
    const svgRect = svgElement.getBoundingClientRect()
    const xAxisRect = xAxisGroup.getBoundingClientRect()
    const containerRect = container.getBoundingClientRect()
    
    const plotLeft = xAxisRect.left - svgRect.left
    const plotWidth = xAxisRect.width
    const dataPointWidth = plotWidth / chartData.length

    const startTimestamp = Math.min(selectionStart, selectionEnd)
    const endTimestamp = Math.max(selectionStart, selectionEnd)

    const startIndex = chartData.findIndex(item => item.timestamp >= startTimestamp)
    const endIndex = chartData.findIndex(item => item.timestamp >= endTimestamp)

    if (startIndex === -1 || endIndex === -1) return null

    const left = svgRect.left - containerRect.left + plotLeft + startIndex * dataPointWidth
    const width = (endIndex - startIndex + 1) * dataPointWidth

    return { left, width }
  }, [isSelecting, selectionStart, selectionEnd, chartData])

  return (
    <Card className="border-border bg-card">
      <CardHeader>
        <CardTitle className="text-card-foreground">Notification Analytics</CardTitle>
        <CardDescription>Notifications grouped by time and urgency level. Click and drag on the chart to select a time range.</CardDescription>
      </CardHeader>
      <CardContent>
        <div
          ref={chartContainerRef}
          className="relative"
          style={{ cursor: isSelecting ? 'crosshair' : 'default' }}
          onMouseDown={handleMouseDown}
        >
          <ResponsiveContainer width="100%" height={350}>
            <BarChart
              data={chartData}
              margin={{
                top: 20,
                right: 30,
                left: 20,
                bottom: 5,
              }}
            >
              <CartesianGrid strokeDasharray="3 3" stroke="#444" opacity={0.5} />
              <XAxis dataKey="time" stroke="#9ca3af" fontSize={12} tick={{ fill: "#9ca3af" }} />
              <YAxis stroke="#9ca3af" fontSize={12} tick={{ fill: "#9ca3af" }} allowDecimals={false} />
              <Tooltip
                contentStyle={{
                  backgroundColor: "#1f2937",
                  borderColor: "#374151",
                  borderRadius: "0.5rem",
                  color: "#f3f4f6",
                }}
                labelStyle={{ color: "#f3f4f6" }}
                itemStyle={{ color: "#f3f4f6" }}
              />
              <Legend wrapperStyle={{ color: "#f3f4f6" }} iconType="rect" />
              <Bar dataKey="Low" fill="#10b981" />
              <Bar dataKey="Normal" fill="#3b82f6" />
              <Bar dataKey="Critical" fill="#ef4444" />
            </BarChart>
          </ResponsiveContainer>
          {selectionOverlay && (
            <div
              className="absolute top-0 bottom-0 bg-primary/20 border-l-2 border-r-2 border-primary pointer-events-none z-10"
              style={{
                left: `${selectionOverlay.left}px`,
                width: `${selectionOverlay.width}px`,
              }}
            />
          )}
        </div>
      </CardContent>
    </Card>
  )
}
