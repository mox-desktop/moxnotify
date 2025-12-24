"use client"

import { useState, useEffect } from "react"
import { Button } from "@/components/ui/button"
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Calendar } from "@/components/ui/calendar"
import { ChevronDown, CalendarIcon, Clock } from "lucide-react"
import { cn } from "@/lib/utils"
import { format } from "date-fns"

export type TimeRangeType = "relative" | "absolute"

export interface RelativeTimeRange {
  type: "relative"
  value: number | "all" // minutes or "all"
}

export interface AbsoluteTimeRange {
  type: "absolute"
  from: Date
  to: Date
}

export type TimeRange = RelativeTimeRange | AbsoluteTimeRange

export interface TimeRangeOption {
  value: number | "all"
  label: string
  minutes: number | null // null for "all"
}

const QUICK_TIME_RANGES: TimeRangeOption[] = [
  { value: 15, label: "Last 15 minutes", minutes: 15 },
  { value: 30, label: "Last 30 minutes", minutes: 30 },
  { value: 60, label: "Last 1 hour", minutes: 60 },
  { value: 120, label: "Last 2 hours", minutes: 120 },
  { value: 240, label: "Last 4 hours", minutes: 240 },
  { value: 360, label: "Last 6 hours", minutes: 360 },
  { value: 720, label: "Last 12 hours", minutes: 720 },
  { value: 1440, label: "Last 1 day", minutes: 1440 },
  { value: 2880, label: "Last 2 days", minutes: 2880 },
  { value: 4320, label: "Last 3 days", minutes: 4320 },
  { value: 10080, label: "Last 1 week", minutes: 10080 },
  { value: 20160, label: "Last 2 weeks", minutes: 20160 },
  { value: 43200, label: "Last 1 month", minutes: 43200 },
  { value: 129600, label: "Last 3 months", minutes: 129600 },
  { value: 259200, label: "Last 6 months", minutes: 259200 },
  { value: 525600, label: "Last 1 year", minutes: 525600 },
  { value: "all", label: "All time", minutes: null },
]

function formatTimeRange(range: TimeRange): string {
  if (range.type === "relative") {
    if (range.value === "all") {
      return "All time"
    }
    const option = QUICK_TIME_RANGES.find(r => r.value === range.value)
    if (option) {
      return option.label
    }
    return formatMinutes(range.value)
  } else {
    // Absolute range
    const fromStr = format(range.from, "MMM d, yyyy HH:mm")
    const toStr = format(range.to, "MMM d, yyyy HH:mm")
    return `${fromStr} - ${toStr}`
  }
}

function formatMinutes(minutes: number): string {
  if (minutes < 60) {
    return `Last ${minutes} minutes`
  } else if (minutes < 1440) {
    const hours = Math.floor(minutes / 60)
    return `Last ${hours} ${hours === 1 ? "hour" : "hours"}`
  } else if (minutes < 10080) {
    const days = Math.floor(minutes / 1440)
    return `Last ${days} ${days === 1 ? "day" : "days"}`
  } else if (minutes < 43200) {
    const weeks = Math.floor(minutes / 10080)
    return `Last ${weeks} ${weeks === 1 ? "week" : "weeks"}`
  } else if (minutes < 129600) {
    const months = Math.floor(minutes / 43200)
    return `Last ${months} ${months === 1 ? "month" : "months"}`
  } else {
    const years = Math.floor(minutes / 525600)
    return `Last ${years} ${years === 1 ? "year" : "years"}`
  }
}

function formatTime(date: Date): string {
  return format(date, "HH:mm")
}

function parseTime(timeStr: string): { hours: number; minutes: number } | null {
  const match = timeStr.match(/^(\d{1,2}):(\d{2})$/)
  if (!match) return null
  const hours = parseInt(match[1], 10)
  const minutes = parseInt(match[2], 10)
  if (hours < 0 || hours > 23 || minutes < 0 || minutes > 59) return null
  return { hours, minutes }
}

interface TimeRangePickerProps {
  value: TimeRange
  onChange: (range: TimeRange) => void
  className?: string
}

export function TimeRangePicker({
  value,
  onChange,
  className,
}: TimeRangePickerProps) {
  const [open, setOpen] = useState(false)
  const [rangeType, setRangeType] = useState<TimeRangeType>(value.type)
  
  // For relative ranges
  const [relativeValue, setRelativeValue] = useState<number | "all">(
    value.type === "relative" ? value.value : 30
  )
  const [customMinutes, setCustomMinutes] = useState("")
  
  // For absolute ranges
  const [fromDate, setFromDate] = useState<Date>(
    value.type === "absolute" ? value.from : new Date(Date.now() - 6 * 60 * 60 * 1000)
  )
  const [toDate, setToDate] = useState<Date>(
    value.type === "absolute" ? value.to : new Date()
  )
  const [fromTime, setFromTime] = useState(formatTime(fromDate))
  const [toTime, setToTime] = useState(formatTime(toDate))
  const [fromCalendarOpen, setFromCalendarOpen] = useState(false)
  const [toCalendarOpen, setToCalendarOpen] = useState(false)

  // Sync state when value prop changes externally
  useEffect(() => {
    if (value.type === "relative") {
      setRangeType("relative")
      setRelativeValue(value.value)
      if (typeof value.value === "number" && !QUICK_TIME_RANGES.find(r => r.value === value.value)) {
        setCustomMinutes(value.value.toString())
      }
    } else {
      setRangeType("absolute")
      setFromDate(value.from)
      setToDate(value.to)
      setFromTime(formatTime(value.from))
      setToTime(formatTime(value.to))
    }
  }, [value])

  const handleQuickSelect = (optionValue: number | "all") => {
    const newRange: RelativeTimeRange = { type: "relative", value: optionValue }
    onChange(newRange)
    setRangeType("relative")
    setRelativeValue(optionValue)
    setOpen(false)
  }

  const handleCustomMinutesSubmit = () => {
    const minutes = Number(customMinutes)
    if (!isNaN(minutes) && minutes > 0) {
      const newRange: RelativeTimeRange = { type: "relative", value: minutes }
      onChange(newRange)
      setRangeType("relative")
      setRelativeValue(minutes)
      setOpen(false)
    }
  }

  const handleAbsoluteRangeApply = () => {
    const fromParsed = parseTime(fromTime)
    const toParsed = parseTime(toTime)
    
    if (!fromParsed || !toParsed) {
      return // Invalid time format
    }

    const from = new Date(fromDate)
    from.setHours(fromParsed.hours, fromParsed.minutes, 0, 0)
    
    const to = new Date(toDate)
    to.setHours(toParsed.hours, toParsed.minutes, 59, 999)

    if (from >= to) {
      return // Invalid range
    }

    const newRange: AbsoluteTimeRange = { type: "absolute", from, to }
    onChange(newRange)
    setRangeType("absolute")
    setOpen(false)
  }

  const handleDateSelect = (date: Date | undefined, type: "from" | "to") => {
    if (!date) return
    
    const parsed = type === "from" ? parseTime(fromTime) : parseTime(toTime)
    
    if (type === "from") {
      const newDate = new Date(date)
      if (parsed) {
        newDate.setHours(parsed.hours, parsed.minutes, 0, 0)
      } else {
        newDate.setHours(0, 0, 0, 0)
      }
      setFromDate(newDate)
    } else {
      const newDate = new Date(date)
      if (parsed) {
        newDate.setHours(parsed.hours, parsed.minutes, 59, 999)
      } else {
        newDate.setHours(23, 59, 59, 999)
      }
      setToDate(newDate)
    }
  }

  const handleTimeChange = (timeStr: string, type: "from" | "to") => {
    const parsed = parseTime(timeStr)
    if (!parsed) {
      // Still update the input even if invalid (user might be typing)
      if (type === "from") {
        setFromTime(timeStr)
      } else {
        setToTime(timeStr)
      }
      return
    }
    
    if (type === "from") {
      setFromTime(timeStr)
      const newDate = new Date(fromDate)
      newDate.setHours(parsed.hours, parsed.minutes, 0, 0)
      setFromDate(newDate)
    } else {
      setToTime(timeStr)
      const newDate = new Date(toDate)
      newDate.setHours(parsed.hours, parsed.minutes, 59, 999)
      setToDate(newDate)
    }
  }

  const displayValue = formatTimeRange(value)
  const fromParsed = parseTime(fromTime)
  const toParsed = parseTime(toTime)
  
  // Check if absolute range is valid
  let canApplyAbsolute = false
  if (fromParsed && toParsed) {
    const from = new Date(fromDate)
    from.setHours(fromParsed.hours, fromParsed.minutes, 0, 0)
    const to = new Date(toDate)
    to.setHours(toParsed.hours, toParsed.minutes, 59, 999)
    canApplyAbsolute = from < to
  }

  return (
    <Popover open={open} onOpenChange={setOpen}>
      <PopoverTrigger asChild>
        <Button
          variant="outline"
          className={cn(
            "h-9 justify-between gap-2 px-3 text-sm font-normal min-w-[280px]",
            className
          )}
        >
          <span className="truncate">{displayValue}</span>
          <ChevronDown className="h-4 w-4 shrink-0 opacity-50" />
        </Button>
      </PopoverTrigger>
      <PopoverContent className="w-[600px] p-4" align="start">
        <div className="space-y-4">
          {/* Range Type Tabs */}
          <div className="flex gap-2 border-b">
            <Button
              variant={rangeType === "relative" ? "default" : "ghost"}
              size="sm"
              className="rounded-none border-b-2 border-transparent data-[variant=default]:border-primary"
              onClick={() => setRangeType("relative")}
            >
              Relative time
            </Button>
            <Button
              variant={rangeType === "absolute" ? "default" : "ghost"}
              size="sm"
              className="rounded-none border-b-2 border-transparent data-[variant=default]:border-primary"
              onClick={() => setRangeType("absolute")}
            >
              Absolute time
            </Button>
          </div>

          {rangeType === "relative" ? (
            <>
              {/* Quick Ranges */}
              <div className="space-y-2">
                <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                  Quick ranges
                </Label>
                <div className="grid grid-cols-2 gap-1.5">
                  {QUICK_TIME_RANGES.map((option) => (
                    <Button
                      key={option.value}
                      variant={value.type === "relative" && relativeValue === option.value ? "default" : "ghost"}
                      size="sm"
                      className={cn(
                        "h-8 justify-start text-xs font-normal",
                        value.type === "relative" && relativeValue === option.value && "bg-primary text-primary-foreground"
                      )}
                      onClick={() => handleQuickSelect(option.value)}
                    >
                      {option.label}
                    </Button>
                  ))}
                </div>
              </div>
              
              {/* Custom Minutes */}
              <div className="space-y-2 border-t pt-3">
                <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                  Custom time range
                </Label>
                <div className="flex items-center gap-2">
                  <Input
                    type="number"
                    min="1"
                    placeholder="Minutes"
                    value={customMinutes}
                    onChange={(e) => setCustomMinutes(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        handleCustomMinutesSubmit()
                      }
                    }}
                    className="h-8 text-xs"
                  />
                  <Button
                    size="sm"
                    className="h-8 px-3 text-xs"
                    onClick={handleCustomMinutesSubmit}
                    disabled={!customMinutes || Number(customMinutes) <= 0}
                  >
                    Apply
                  </Button>
                </div>
              </div>
            </>
          ) : (
            <>
              {/* Absolute Time Range */}
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                    From
                  </Label>
                  <div className="flex items-center gap-2">
                    <Popover open={fromCalendarOpen} onOpenChange={setFromCalendarOpen}>
                      <PopoverTrigger asChild>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-9 flex-1 justify-start text-left font-normal"
                        >
                          <CalendarIcon className="mr-2 h-4 w-4" />
                          {format(fromDate, "MMM d, yyyy")}
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent className="w-auto p-0" align="start">
                        <Calendar
                          mode="single"
                          selected={fromDate}
                          onSelect={(date) => {
                            if (date) {
                              handleDateSelect(date, "from")
                              setFromCalendarOpen(false)
                            }
                          }}
                          initialFocus
                        />
                      </PopoverContent>
                    </Popover>
                    <div className="flex items-center gap-1">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                      <Input
                        type="time"
                        value={fromTime}
                        onChange={(e) => handleTimeChange(e.target.value, "from")}
                        className="h-9 w-[100px]"
                      />
                    </div>
                  </div>
                </div>

                <div className="space-y-2">
                  <Label className="text-xs font-semibold text-muted-foreground uppercase tracking-wide">
                    To
                  </Label>
                  <div className="flex items-center gap-2">
                    <Popover open={toCalendarOpen} onOpenChange={setToCalendarOpen}>
                      <PopoverTrigger asChild>
                        <Button
                          variant="outline"
                          size="sm"
                          className="h-9 flex-1 justify-start text-left font-normal"
                        >
                          <CalendarIcon className="mr-2 h-4 w-4" />
                          {format(toDate, "MMM d, yyyy")}
                        </Button>
                      </PopoverTrigger>
                      <PopoverContent className="w-auto p-0" align="start">
                        <Calendar
                          mode="single"
                          selected={toDate}
                          onSelect={(date) => {
                            if (date) {
                              handleDateSelect(date, "to")
                              setToCalendarOpen(false)
                            }
                          }}
                          initialFocus
                        />
                      </PopoverContent>
                    </Popover>
                    <div className="flex items-center gap-1">
                      <Clock className="h-4 w-4 text-muted-foreground" />
                      <Input
                        type="time"
                        value={toTime}
                        onChange={(e) => handleTimeChange(e.target.value, "to")}
                        className="h-9 w-[100px]"
                      />
                    </div>
                  </div>
                </div>

                <Button
                  size="sm"
                  className="w-full"
                  onClick={handleAbsoluteRangeApply}
                  disabled={!canApplyAbsolute}
                >
                  Apply
                </Button>
              </div>
            </>
          )}
        </div>
      </PopoverContent>
    </Popover>
  )
}
