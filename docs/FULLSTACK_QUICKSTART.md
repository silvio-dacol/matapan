# Quick Start Guide - Running the Full Stack

This guide will help you run both the Rust backend API and the Next.js frontend together.

## Prerequisites

- Rust toolchain installed
- Node.js (v18 or higher) and npm installed
- Git repository cloned

## Step 1: Start the Backend API

Open a terminal and run:

```powershell
# From the project root
cd c:\Users\Silvio\git-repos\net-worth

# Run the backend server
.\run-api.ps1
```

Or manually:

```powershell
cargo run --release --bin server
```

The backend will start on `http://localhost:3000`

Verify it's running by visiting: `http://localhost:3000/api/dashboard`

## Step 2: Start the Frontend

Open a **second terminal** and run:

```powershell
# Navigate to frontend directory
cd c:\Users\Silvio\git-repos\net-worth\frontend

# Install dependencies (first time only)
npm install

# Start the development server
npm run dev
```

The frontend will start on `http://localhost:3001` (or next available port)

## Step 3: Access the Dashboard

Open your browser and navigate to:

```
http://localhost:3001
```

You should see:

- ✅ Net worth metrics in the top cards
- ✅ Asset breakdown pie chart
- ✅ Net worth over time area chart
- ✅ Snapshot history table
- ✅ "Last updated" timestamp

## Features to Test

### Auto-Refresh

- The dashboard polls the backend every 30 seconds
- Watch the "Last updated" timestamp change
- Data updates automatically without page reload

### Manual Refresh

- Click the "Refresh" button in the top-right
- Forces an immediate data fetch
- Useful after updating database JSON files

### Interactive Charts

- Hover over chart elements to see tooltips
- Check the pie chart for asset distribution
- Review the area chart for historical trends

### Snapshot Table

- Scroll through all monthly snapshots
- Latest month is highlighted with a "Latest" badge
- All values are formatted in EUR

## Troubleshooting

### Backend Not Running

**Error**: "Error Loading Dashboard" - "Failed to fetch dashboard data"

**Solution**: Make sure the backend is running on port 3000

```powershell
# Check if running
Get-Process | Where-Object {$_.ProcessName -like "*server*"}

# Restart backend
.\run-api.ps1
```

### Port Already in Use

**Error**: "Port 3000 is already in use"

**Solution**: Kill the process or use a different port

```powershell
# Find process on port 3000
netstat -ano | findstr :3000

# Kill process (replace <PID> with actual process ID)
taskkill /PID <PID> /F
```

### TypeScript Errors in Frontend

**Error**: "Cannot find module '@/components/...'"

**Solution**: Clear TypeScript cache and restart

```powershell
# Delete TypeScript cache
rm -r frontend\.next

# Restart dev server
npm run dev
```

### CORS Errors

**Error**: "CORS policy: No 'Access-Control-Allow-Origin' header"

**Solution**: This shouldn't happen if using the Next.js proxy. Check:

1. Frontend is using `/api` (not `http://localhost:3000`)
2. `next.config.ts` has the rewrite rule
3. Backend is running on port 3000

### Charts Not Displaying

**Error**: Charts are blank or missing

**Solution**: Check browser console for errors

```powershell
# Verify Recharts is installed
npm list recharts

# Reinstall if needed
npm install recharts
```

## Development Workflow

### Making Changes

**Backend Changes** (Rust):

1. Edit files in `crates/backend_api/src/`
2. Backend auto-reloads (if using cargo-watch)
3. Or manually restart: `.\run-api.ps1`

**Frontend Changes** (React/TypeScript):

1. Edit files in `frontend/`
2. Next.js hot-reloads automatically
3. Changes appear immediately in browser

**Data Changes**:

1. Edit JSON files in `database/`
2. Restart backend or use cache invalidate endpoint
3. Frontend will pick up changes on next poll

### Adding New Features

**New API Endpoint**:

1. Add route in `crates/backend_api/src/router.rs`
2. Add handler in `crates/backend_api/src/handlers.rs`
3. Update frontend `lib/api.ts` with new function
4. Create hook in `hooks/use-dashboard.ts` if needed

**New Dashboard Component**:

1. Create component in `frontend/components/dashboard/`
2. Import in `app/page.tsx`
3. Use existing hooks for data
4. Style with Tailwind CSS

## Production Deployment

### Backend

```powershell
# Build optimized binary
cargo build --release

# Binary location
.\target\release\server.exe
```

### Frontend

```powershell
cd frontend

# Build for production
npm run build

# Test production build locally
npm start
```

### Environment Variables

**Frontend** (`.env.local`):

```bash
NEXT_PUBLIC_API_URL=https://your-api-domain.com
```

**Backend** (`settings.json`):
Update paths and configuration as needed

## Next Steps

- 📱 Test responsive design on mobile
- 🎨 Customize colors in `tailwind.config.ts`
- 📊 Add more charts or visualizations
- 🔐 Add authentication if needed
- 🚀 Deploy to production hosting
- 📈 Set up monitoring and analytics

## Useful Commands

```powershell
# Backend
cargo test                    # Run tests
cargo fmt                     # Format code
cargo clippy                  # Lint code

# Frontend
npm run lint                  # Lint code
npm run build                 # Build for production
npm run dev -- --turbo        # Use Turbopack (faster)
npx shadcn@latest add button  # Add UI component

# Full stack
# Terminal 1: Backend
.\run-api.ps1

# Terminal 2: Frontend
cd frontend && npm run dev
```

## Support

For issues or questions:

1. Check the main README.md
2. Review API documentation in `docs/API_QUICKSTART.md`
3. Check backend logs for errors
4. Inspect browser console for frontend errors
