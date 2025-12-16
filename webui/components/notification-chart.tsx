"use client"

import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card"
import { BarChart, Bar, XAxis, YAxis, CartesianGrid, Tooltip, Legend, ResponsiveContainer } from "recharts"
import type { DbusNotification } from "@/lib/types"
import { useMemo } from "react"

interface NotificationChartProps {
  notifications: DbusNotification[]
  timeInterval: number // in minutes
  timeRange?: number | "all" // in minutes, or "all" for no limit
}

export function NotificationChart({ notifications, timeInterval, timeRange }: NotificationChartProps) {
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
    
    if (timeRange === "all" || !timeRange) {
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
    } else {
      // Use the specified time range (show last N minutes)
      // Round endTime down to match how notifications are bucketed (rounded down)
      endTime = Math.floor(now / intervalMs) * intervalMs
      startTime = endTime - (timeRange * 60 * 1000)
      startTime = Math.floor(startTime / intervalMs) * intervalMs
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

  return (
    <Card className="border-border bg-card">
      <CardHeader>
        <CardTitle className="text-card-foreground">Notification Analytics</CardTitle>
        <CardDescription>Notifications grouped by time and urgency level</CardDescription>
      </CardHeader>
      <CardContent>
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
      </CardContent>
    </Card>
  )
}
