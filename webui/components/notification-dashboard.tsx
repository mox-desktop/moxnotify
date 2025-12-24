"use client"

import { useState, useEffect, useCallback } from "react"
import { useSearchParams, useRouter, usePathname } from "next/navigation"
import { NotificationChart } from "./notification-chart"
import { NotificationList } from "./notification-list"
import { SearchBar } from "./search-bar"
import { TimeRangePicker, type TimeRange, type RelativeTimeRange, type AbsoluteTimeRange } from "./time-range-picker"
import type { DbusNotification } from "@/lib/types"
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select"
import { Label } from "@/components/ui/label"
import { Loader2 } from "lucide-react"

const API_BASE_URL = process.env.NEXT_PUBLIC_API_URL || "http://localhost:3029"

export function NotificationDashboard() {
  const searchParams = useSearchParams()
  const router = useRouter()
  const pathname = usePathname()

  // Initialize state from URL parameters
  const [searchInput, setSearchInput] = useState(searchParams.get("q") || "*")
  const [searchQuery, setSearchQuery] = useState(searchParams.get("q") || "*")
  const [notifications, setNotifications] = useState<DbusNotification[]>([])
  const [isLoading, setIsLoading] = useState(false)
  const [error, setError] = useState<string | null>(null)
  const [timeInterval, setTimeInterval] = useState(
    Number(searchParams.get("interval")) || 5
  )
  // Parse time range from URL params
  const getInitialTimeRange = (): TimeRange => {
    const relativeParam = searchParams.get("relative")
    const fromParam = searchParams.get("from")
    const toParam = searchParams.get("to")
    
    // Check for relative range first
    if (relativeParam) {
      if (relativeParam === "all") {
        return { type: "relative", value: "all" }
      }
      const minutes = Number(relativeParam)
      if (!isNaN(minutes) && minutes > 0) {
        return { type: "relative", value: minutes }
      }
    }
    
    // Check for absolute range
    if (fromParam && toParam) {
      const from = new Date(fromParam)
      const to = new Date(toParam)
      if (!isNaN(from.getTime()) && !isNaN(to.getTime()) && from < to) {
        return { type: "absolute", from, to }
      }
    }
    
    // Default: last 30 minutes
    return { type: "relative", value: 30 }
  }
  
  const [timeRange, setTimeRange] = useState<TimeRange>(getInitialTimeRange())
  const [maxHits, setMaxHits] = useState<number>(
    searchParams.get("maxHits") ? Number(searchParams.get("maxHits")) : 100
  )
  const [sortBy, setSortBy] = useState<string>(searchParams.get("sortBy") || "id")
  const [sortOrder, setSortOrder] = useState<"asc" | "desc">(
    (searchParams.get("sortOrder") as "asc" | "desc") || "desc"
  )
  const [refetchTrigger, setRefetchTrigger] = useState(0)

  // Fetch notifications from API
  const fetchNotifications = useCallback(async () => {
    setIsLoading(true)
    setError(null)

    try {
      // Build request body with timestamp filters for absolute ranges
      const requestBody: any = {
        query: searchQuery || "*",
        max_hits: maxHits,
        sort_by: sortBy,
        sort_order: sortOrder,
      }

      // Add timestamp filters for absolute time ranges
      if (timeRange.type === "absolute") {
        requestBody.start_timestamp = timeRange.from.toISOString()
        requestBody.end_timestamp = timeRange.to.toISOString()
      }

      const response = await fetch(`${API_BASE_URL}/api/search`, {
        method: "POST",
        headers: {
          "Content-Type": "application/json",
        },
        body: JSON.stringify(requestBody),
      })

      if (!response.ok) {
        let errorMessage = `API error: ${response.statusText}`
        try {
          const errorData = await response.json()
          if (errorData.error || errorData.message) {
            errorMessage = errorData.error || errorData.message
          }
        } catch {
          // If response is not JSON, use status text
        }
        throw new Error(errorMessage)
      }

      let data
      try {
        data = await response.json()
      } catch (err) {
        throw new Error("Invalid JSON response from API")
      }
      
      // Handle different response formats
      let notificationsArray: any[] = []
      if (Array.isArray(data)) {
        notificationsArray = data
      } else if (data.notifications && Array.isArray(data.notifications)) {
        notificationsArray = data.notifications
      } else if (data.results && Array.isArray(data.results)) {
        notificationsArray = data.results
      } else {
        throw new Error("Unexpected API response format")
      }
      
      // Convert timestamp strings to Date objects if needed
      const notificationsWithDates = notificationsArray.map((n: any) => {
        try {
          // Preserve all fields including expire_timeout (handle different field name variations)
          const expireTimeout = n.expire_timeout !== undefined 
            ? n.expire_timeout 
            : n.expireTimeout !== undefined 
            ? n.expireTimeout 
            : n.timeout !== undefined
            ? n.timeout
            : undefined
          
          // Extract urgency from hints.urgency (always 0, 1, or 2)
          let urgency: 0 | 1 | 2 = 1 // Default to Normal (1)
          if (n.hints) {
            let hintsObj = n.hints
            // Handle hints as array containing JSON string (API format)
            if (Array.isArray(hintsObj) && hintsObj.length > 0 && typeof hintsObj[0] === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj[0])
              } catch (e) {
                console.warn(`Failed to parse hints array[0] as JSON for notification ${n.id}:`, e)
              }
            }
            // If hints is a string, parse it
            else if (typeof hintsObj === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj)
              } catch (e) {
                console.warn(`Failed to parse hints as JSON for notification ${n.id}:`, e)
              }
            }
            
            // Extract urgency from hints object
            if (hintsObj && typeof hintsObj === 'object' && hintsObj !== null && !Array.isArray(hintsObj)) {
              // Check if urgency exists and is a number
              if ('urgency' in hintsObj && typeof hintsObj.urgency === 'number') {
                const urgencyValue = hintsObj.urgency
                if (urgencyValue === 0 || urgencyValue === 1 || urgencyValue === 2) {
                  urgency = urgencyValue as 0 | 1 | 2
                } else {
                  console.warn(`Invalid urgency value ${urgencyValue} for notification ${n.id}`)
                }
              } else {
                console.warn(`No valid urgency found in hints for notification ${n.id}:`, hintsObj)
              }
            }
            
            // Debug: log urgency extraction for first few notifications
            if (notificationsArray.indexOf(n) < 3) {
              console.log(`Notification ${n.id}: hints=`, n.hints, `parsed hints=`, hintsObj, `urgency=`, urgency)
            }
          } else {
            // Debug: log when hints is missing
            if (notificationsArray.indexOf(n) < 3) {
              console.warn(`Notification ${n.id} has no hints field`)
            }
          }
          
          const notification = {
            ...n,
            timestamp: n.timestamp ? new Date(n.timestamp) : new Date(),
            // Explicitly set expire_timeout to ensure it's preserved
            expire_timeout: expireTimeout,
            // Explicitly set urgency to ensure it's preserved and validated
            urgency: urgency,
          }
          
          // Debug: verify urgency is set correctly
          if (notificationsArray.indexOf(n) < 3) {
            console.log(`Final notification ${notification.id}: urgency=${notification.urgency}`)
          }
          
          return notification
        } catch (err) {
          console.error("Error parsing notification:", n, err)
          const expireTimeout = n.expire_timeout !== undefined 
            ? n.expire_timeout 
            : n.expireTimeout !== undefined 
            ? n.expireTimeout 
            : n.timeout
          
          // Extract urgency from hints.urgency
          let urgency: 0 | 1 | 2 = 1 // Default to Normal (1)
          if (n.hints) {
            let hintsObj = n.hints
            // Handle hints as array containing JSON string (API format)
            if (Array.isArray(hintsObj) && hintsObj.length > 0 && typeof hintsObj[0] === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj[0])
              } catch (e) {
                // If parsing fails, use default
              }
            }
            // If hints is a string, parse it
            else if (typeof hintsObj === 'string') {
              try {
                hintsObj = JSON.parse(hintsObj)
              } catch (e) {
                // If parsing fails, use default
              }
            }
            
            if (hintsObj && typeof hintsObj === 'object' && hintsObj !== null && !Array.isArray(hintsObj) && typeof hintsObj.urgency === 'number') {
              const urgencyValue = hintsObj.urgency
              if (urgencyValue === 0 || urgencyValue === 1 || urgencyValue === 2) {
                urgency = urgencyValue as 0 | 1 | 2
              }
            }
          }
          
          return {
            ...n,
            timestamp: new Date(),
            expire_timeout: expireTimeout,
            urgency: urgency,
          }
        }
      })
      
      // Filter notifications by time range if specified (only for relative ranges, absolute is handled by backend)
      let filteredNotifications = notificationsWithDates
      
      if (timeRange.type === "relative") {
        if (timeRange.value !== "all") {
          const now = Date.now()
          const timeRangeMs = timeRange.value * 60 * 1000
          filteredNotifications = notificationsWithDates.filter((n) => {
            const notificationTime = n.timestamp instanceof Date 
              ? n.timestamp.getTime() 
              : new Date(n.timestamp).getTime()
            return now - notificationTime <= timeRangeMs
          })
        }
      }
      // For absolute ranges, backend handles filtering via start_timestamp/end_timestamp
      
      setNotifications(filteredNotifications)
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : "Failed to fetch notifications"
      console.error("Error fetching notifications:", err)
      setError(errorMessage)
      setNotifications([])
    } finally {
      setIsLoading(false)
    }
  }, [searchQuery, maxHits, sortBy, sortOrder, timeRange])

  // Fetch on mount and when search parameters change
  useEffect(() => {
    fetchNotifications()
  }, [fetchNotifications, refetchTrigger])

  // Update URL when state changes
  useEffect(() => {
    const params = new URLSearchParams()

    if (searchQuery.trim() && searchQuery !== "*") {
      params.set("q", searchQuery)
    }

    if (timeInterval !== 5) {
      params.set("interval", timeInterval.toString())
    }

    // Handle time range URL params
    if (timeRange.type === "absolute") {
      params.set("from", timeRange.from.toISOString())
      params.set("to", timeRange.to.toISOString())
    } else {
      // Relative range
      if (timeRange.value === "all") {
        params.set("relative", "all")
      } else if (timeRange.value !== 30) {
        // Only set if not default (30 minutes)
        params.set("relative", timeRange.value.toString())
      }
    }

    if (maxHits !== 100) {
      params.set("maxHits", maxHits.toString())
    }

    if (sortBy !== "id") {
      params.set("sortBy", sortBy)
    }

    if (sortOrder !== "desc") {
      params.set("sortOrder", sortOrder)
    }

    const newUrl = params.toString() ? `${pathname}?${params.toString()}` : pathname
    router.replace(newUrl, { scroll: false })
  }, [searchQuery, timeInterval, timeRange, maxHits, sortBy, sortOrder, pathname, router])

  const handleRunSearch = () => {
    const newQuery = searchInput.trim() || "*"
    setSearchQuery(newQuery)
    // Force refetch even if query didn't change by incrementing refetch trigger
    setRefetchTrigger((prev) => prev + 1)
  }

  const handleTimeIntervalChange = (value: string) => {
    setTimeInterval(Number(value))
  }

  const handleTimeRangeChange = (range: TimeRange) => {
    setTimeRange(range)
  }

  const handleMaxHitsChange = (value: string) => {
    setMaxHits(Number(value))
  }

  const handleSortByChange = (value: string) => {
    setSortBy(value)
  }

  const handleSortOrderChange = (value: string) => {
    setSortOrder(value as "asc" | "desc")
  }

  return (
    <div className="mx-auto max-w-7xl p-4 md:p-6 lg:p-8">
      <header className="mb-6">
        <h1 className="text-3xl font-bold tracking-tight text-foreground">Notification Center</h1>
        <p className="mt-1 text-sm text-muted-foreground">D-Bus specification compliant notification viewer</p>
      </header>

      <div className="space-y-6">
        <div className="flex flex-wrap items-center gap-3">
          <div className="flex items-center gap-3">
            <Label htmlFor="interval-select" className="text-sm font-medium text-foreground">
              Chart Time Interval:
            </Label>
            <Select value={timeInterval.toString()} onValueChange={handleTimeIntervalChange}>
              <SelectTrigger id="interval-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="5">5 minutes</SelectItem>
                <SelectItem value="10">10 minutes</SelectItem>
                <SelectItem value="15">15 minutes</SelectItem>
                <SelectItem value="20">20 minutes</SelectItem>
                <SelectItem value="30">30 minutes</SelectItem>
                <SelectItem value="60">1 hour</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="time-range-picker" className="text-sm font-medium text-foreground">
              Display Time Range:
            </Label>
            <TimeRangePicker
              value={timeRange}
              onChange={handleTimeRangeChange}
            />
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="max-hits-select" className="text-sm font-medium text-foreground">
              Max Hits:
            </Label>
            <Select
              value={maxHits.toString()}
              onValueChange={handleMaxHitsChange}
            >
              <SelectTrigger id="max-hits-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="10">10</SelectItem>
                <SelectItem value="25">25</SelectItem>
                <SelectItem value="50">50</SelectItem>
                <SelectItem value="100">100</SelectItem>
                <SelectItem value="200">200</SelectItem>
                <SelectItem value="500">500</SelectItem>
                <SelectItem value="1000">1000</SelectItem>
                <SelectItem value="2000">2000</SelectItem>
                <SelectItem value="5000">5000</SelectItem>
                <SelectItem value="10000">10000</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="sort-by-select" className="text-sm font-medium text-foreground">
              Sort By:
            </Label>
            <Select value={sortBy} onValueChange={handleSortByChange}>
              <SelectTrigger id="sort-by-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="id">ID</SelectItem>
                <SelectItem value="timestamp">Timestamp</SelectItem>
                <SelectItem value="app_name">App Name</SelectItem>
              </SelectContent>
            </Select>
          </div>
          <div className="flex items-center gap-3">
            <Label htmlFor="sort-order-select" className="text-sm font-medium text-foreground">
              Sort Order:
            </Label>
            <Select value={sortOrder} onValueChange={handleSortOrderChange}>
              <SelectTrigger id="sort-order-select" className="w-[180px]">
                <SelectValue />
              </SelectTrigger>
              <SelectContent>
                <SelectItem value="asc">Ascending</SelectItem>
                <SelectItem value="desc">Descending</SelectItem>
              </SelectContent>
            </Select>
          </div>
        </div>

        <NotificationChart 
          notifications={notifications} 
          timeInterval={timeInterval} 
          timeRange={timeRange}
          onTimeRangeSelect={(range) => setTimeRange(range)}
        />
        <SearchBar value={searchInput} onChange={setSearchInput} onRun={handleRunSearch} />
        
        {isLoading && (
          <div className="flex items-center justify-center py-12">
            <Loader2 className="h-8 w-8 animate-spin text-muted-foreground" />
          </div>
        )}
        
        {error && (
          <div className="rounded-lg border border-destructive bg-destructive/10 p-4">
            <p className="text-sm text-destructive">Error: {error}</p>
          </div>
        )}
        
        {!isLoading && !error && (
          <NotificationList notifications={notifications} />
        )}
      </div>
    </div>
  )
}
