# Net Worth Dashboard - Frontend

A modern, responsive Next.js dashboard for visualizing your personal net worth data.

## Tech Stack

- **Framework**: Next.js 16 (App Router)
- **Language**: TypeScript
- **Styling**: Tailwind CSS
- **UI Components**: shadcn/ui
- **Data Fetching**: TanStack Query (React Query)
- **Charts**: Recharts
- **Icons**: Lucide React

## Features

- 📊 **Real-time Data**: Auto-refresh every 30 seconds with polling
- 📈 **Interactive Charts**:
  - Pie chart for asset breakdown
  - Area chart for net worth over time
  - Detailed snapshot table
- 💰 **Key Metrics Dashboard**:
  - Net Worth
  - Total Assets
  - Liabilities
  - Purchasing Power comparison vs New York
- 🎨 **Modern UI**: Clean, responsive design with shadcn/ui components
- 🔄 **Cache Management**: Manual refresh button and automatic cache invalidation

## Getting Started

### Prerequisites

Make sure you have the Rust backend API running on `http://localhost:3000`. See the main project README for backend setup instructions.

### Installation

```bash
# Navigate to the frontend directory
cd frontend

# Install dependencies
npm install
```

### Development

```bash
# Start the development server
npm run dev
```

The dashboard will be available at `http://localhost:3001` (or the next available port).

### Production Build

```bash
# Build for production
npm run build

# Start production server
npm start
```

## Project Structure

```
frontend/
├── app/
│   ├── layout.tsx          # Root layout with QueryProvider
│   ├── page.tsx            # Main dashboard page
│   └── globals.css         # Global styles
├── components/
│   ├── ui/                 # shadcn/ui components
│   └── dashboard/          # Dashboard-specific components
│       ├── assets-breakdown-chart.tsx
│       ├── net-worth-chart.tsx
│       └── snapshot-table.tsx
├── hooks/
│   └── use-dashboard.ts    # React Query hooks for API
├── lib/
│   ├── api.ts             # API client functions
│   ├── types.ts           # TypeScript type definitions
│   ├── query-provider.tsx # React Query provider
│   └── utils.ts           # Utility functions
└── next.config.ts         # Next.js configuration with API proxy
```

## API Integration

The frontend integrates with your Rust backend API through the following endpoints:

- `GET /api/dashboard` - Full dashboard with all snapshots
- `GET /api/dashboard/latest` - Latest snapshot only
- `GET /api/snapshots/:date/entries` - Detailed entries for a specific month
- `POST /api/cache/invalidate` - Force cache refresh

### API Proxy

The Next.js app is configured to proxy `/api/*` requests to `http://localhost:3000` during development. This avoids CORS issues and keeps the backend origin hidden.

To change the backend URL, update `next.config.ts`:

```typescript
async rewrites() {
  return [
    {
      source: '/api/:path*',
      destination: 'http://your-backend-url/:path*',
    },
  ];
}
```

## Configuration

### Environment Variables

Create a `.env.local` file (already included):

```bash
NEXT_PUBLIC_API_URL=/api
```

For production deployments, set this to your actual API URL.

### Auto-refresh Interval

The dashboard polls for updates every 30 seconds by default. To change this, modify the interval in `app/page.tsx`:

```typescript
const { data: dashboard } = useDashboard(60000); // Poll every 60 seconds
```

## Customization

### Styling

The app uses Tailwind CSS with shadcn/ui theming. Customize colors and styles in:

- `tailwind.config.ts` - Tailwind configuration
- `app/globals.css` - CSS variables for theming

### Adding Components

To add new shadcn/ui components:

```bash
npx shadcn@latest add [component-name]
```

Example: `npx shadcn@latest add dialog table dropdown-menu`

## Deployment

### Vercel (Recommended)

1. Push your code to GitHub
2. Import the project in Vercel
3. Set the root directory to `frontend`
4. Configure environment variables
5. Deploy!

### Other Platforms

The app works on any Node.js hosting platform:

- Netlify
- AWS Amplify
- Railway
- Docker

Make sure to:

1. Build the app: `npm run build`
2. Set environment variables
3. Configure API proxy or CORS on backend

## Development Tips

### React Query DevTools

The React Query DevTools are included in development mode. Look for the floating icon in the bottom-right corner to inspect queries and cache state.

### Type Safety

All API responses are strongly typed. If you modify the backend API, update the types in `lib/types.ts` to match.

### Hot Reload

Next.js supports fast refresh. Changes to components will update instantly without losing state.

## Troubleshooting

### API Connection Issues

If you see "Error Loading Dashboard":

1. Ensure the backend is running on `http://localhost:3000`
2. Check that the API endpoints are accessible
3. Look for CORS errors in the browser console
4. Verify the proxy configuration in `next.config.ts`

### Build Errors

If you encounter TypeScript errors:

1. Run `npm run lint` to see all issues
2. Make sure all imported types exist
3. Check that all dependencies are installed

### Chart Not Rendering

If charts don't appear:

1. Check browser console for errors
2. Ensure Recharts is installed: `npm install recharts`
3. Verify data is being fetched correctly

## Future Enhancements

Potential features to add:

- 📱 Mobile-optimized view
- 🌙 Dark mode toggle
- 📊 More chart types (bar charts, scatter plots)
- 🔍 Detailed entry view/modal
- 📥 Export data (CSV, PDF)
- 🎯 Financial goals tracking
- 📈 Trend analysis and projections
- 🔔 Notification system for significant changes

## License

Same as the parent project.
