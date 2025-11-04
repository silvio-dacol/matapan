# Auto-Refresh Dashboard Guide

This guide explains how to make your frontend dashboard automatically update when you regenerate data with `cargo run --bin cli`, **without manually reloading the page**.

## 🎯 Three Approaches

### **Option 1: Polling (Simplest)** ⭐ **RECOMMENDED FOR YOUR USE CASE**

**How it works:** Frontend checks for updates every N seconds by comparing `generated_at` timestamp.

**Pros:**

- ✅ Simple to implement (no backend changes needed!)
- ✅ Works with current API as-is
- ✅ Easy to debug
- ✅ Works across all browsers

**Cons:**

- ⚠️ Slight delay (5 seconds default)
- ⚠️ Some unnecessary requests if data hasn't changed

**Implementation:** See `docs/auto-refresh-polling.tsx`

**When to use:** Your personal dashboard with infrequent updates (perfect for your use case!)

---

### **Option 2: Server-Sent Events (SSE)**

**How it works:** Backend continuously streams updates to frontend when data changes.

**Pros:**

- ✅ Real-time updates
- ✅ Efficient (server pushes only when needed)
- ✅ Built into HTTP

**Cons:**

- ⚠️ Requires backend modifications
- ⚠️ Keeps connection open constantly
- ⚠️ More complex to implement

**Implementation:** See `docs/auto-refresh-sse.tsx`

**When to use:** Multiple users, high-frequency updates, need instant refresh

---

### **Option 3: WebSocket + File Watcher (Most Sophisticated)**

**How it works:** Backend watches `dashboard.json` for changes and pushes notification to all connected clients via WebSocket.

**Pros:**

- ✅ Instant updates (< 1 second after regeneration)
- ✅ Most efficient (only notifies on actual file change)
- ✅ Bi-directional communication possible

**Cons:**

- ⚠️ Requires file watcher dependency (`notify`)
- ⚠️ More complex backend setup
- ⚠️ WebSocket infrastructure

**Implementation:** See `docs/auto-refresh-websocket.tsx`

**When to use:** Production app with many users, need instant updates

---

## 📊 Comparison Table

| Feature             | Polling        | SSE         | WebSocket      |
| ------------------- | -------------- | ----------- | -------------- |
| **Backend Changes** | None           | Moderate    | Significant    |
| **Response Time**   | 5-30 seconds   | < 1 second  | < 1 second     |
| **Server Load**     | Low-Medium     | Medium      | Low            |
| **Complexity**      | ⭐ Easy        | ⭐⭐ Medium | ⭐⭐⭐ Complex |
| **Browser Support** | 100%           | 95%         | 100%           |
| **Best For**        | Personal/Small | Multi-user  | Production     |

---

## 🚀 Quick Start (Recommended: Polling)

### 1. Install in your frontend project:

```bash
npm install
# No additional dependencies needed!
```

### 2. Copy the hook from `docs/auto-refresh-polling.tsx`

### 3. Use in your component:

```tsx
import { useAutoRefreshDashboard } from "./hooks/useAutoRefreshDashboard";

function Dashboard() {
  // Check every 5 seconds (5000ms)
  const { dashboard, lastUpdate } = useAutoRefreshDashboard(5000);

  return (
    <div>
      <div className="last-update">
        Last updated: {new Date(lastUpdate).toLocaleString()}
      </div>

      {/* Your charts automatically update! */}
      <NetWorthChart data={dashboard?.snapshots} />
      <CategoryBreakdown breakdown={dashboard?.snapshots[0]?.breakdown} />
    </div>
  );
}
```

### 4. Test the workflow:

```powershell
# Terminal 1: Start API server
cargo run --bin server

# Terminal 2: Regenerate data
cargo run --bin cli

# Your browser dashboard updates automatically within 5 seconds!
# No page reload needed! 🎉
```

---

## 🔧 Configuration

### Adjust polling interval:

```tsx
// Check every 2 seconds (more responsive, more requests)
const { dashboard } = useAutoRefreshDashboard(2000);

// Check every 30 seconds (less responsive, fewer requests)
const { dashboard } = useAutoRefreshDashboard(30000);
```

### Smart polling (only when visible):

```tsx
useEffect(() => {
  const handleVisibilityChange = () => {
    if (document.hidden) {
      // Pause polling when tab is hidden
    } else {
      // Resume polling when tab is visible
    }
  };

  document.addEventListener("visibilitychange", handleVisibilityChange);
  return () =>
    document.removeEventListener("visibilitychange", handleVisibilityChange);
}, []);
```

---

## 🎬 How It Works (Polling)

1. **Initial Load:** Frontend fetches dashboard and stores `generated_at` timestamp
2. **Polling Loop:** Every 5 seconds, fetch dashboard again
3. **Compare:** Check if new `generated_at` is different
4. **Update:** If changed, update state → React re-renders charts/tables
5. **Visual Feedback:** Show "Last updated" indicator

```
┌─────────────┐         ┌─────────────┐         ┌─────────────┐
│  Frontend   │         │   Backend   │         │ dashboard.  │
│  (React)    │         │   (Axum)    │         │   json      │
└──────┬──────┘         └──────┬──────┘         └──────┬──────┘
       │                       │                       │
       │  GET /api/dashboard   │                       │
       ├──────────────────────>│                       │
       │                       │  read_to_string       │
       │                       ├──────────────────────>│
       │                       │<──────────────────────┤
       │  { generated_at:...} │                       │
       │<──────────────────────┤                       │
       │                       │                       │
       ⏱️  Wait 5 seconds       │                       │
       │                       │                       │
       │  GET /api/dashboard   │                       │
       ├──────────────────────>│                       │
       │                       │  read_to_string       │
       │                       ├──────────────────────>│
       │  Same timestamp       │                       │
       │<──────────────────────┤                       │
       │  (No update needed)   │                       │
       │                       │                       │
  [User runs: cargo run --bin cli]                    │
       │                       │                       │
       │                       │        NEW FILE       │
       │                       │<──────────────────────┤
       │                       │                       │
       ⏱️  Wait 5 seconds       │                       │
       │                       │                       │
       │  GET /api/dashboard   │                       │
       ├──────────────────────>│                       │
       │                       │  read_to_string       │
       │                       ├──────────────────────>│
       │  NEW timestamp! 🎉    │                       │
       │<──────────────────────┤                       │
       │  (Update charts)      │                       │
       │                       │                       │
```

---

## 💡 Tips

1. **Visual Feedback:** Add a "Live" indicator so users know it's auto-updating
2. **Error Handling:** Show connection status in case API is down
3. **Smooth Transitions:** Use CSS transitions for chart updates
4. **Loading States:** Show skeleton loaders during initial fetch

---

## 🎯 My Recommendation for Your Project

Use **Polling (Option 1)** because:

- ✅ No backend changes needed (already done!)
- ✅ Your dashboard is personal/single-user
- ✅ Updates are infrequent (monthly snapshots)
- ✅ 5-second delay is perfectly acceptable
- ✅ Simplest to maintain

If you later deploy for multiple users or need instant updates, upgrade to WebSocket + File Watcher.

---

## 📝 Next Steps

1. Copy `useAutoRefreshDashboard` hook to your frontend
2. Wrap your main Dashboard component with it
3. Add a "Last updated" timestamp display
4. Test: Run `cargo run --bin cli` and watch your charts update automatically!

Enjoy your self-updating dashboard! 🎉
