import { LocationProvider } from "preact-iso";
import { PrinterDiscoveryProvider } from "./app/printer-discovery-data";
import { PrinterInventoryProvider } from "./app/printer-inventory-data";
import { ProfileDataProvider } from "./app/profile-data";
import { AppRoutes } from "./app/routes";
import { ServerStatusProvider } from "./app/server-status-data";
import { AppShell } from "./app/shell";

export function App() {
  return (
    <ServerStatusProvider>
      <PrinterInventoryProvider>
        <PrinterDiscoveryProvider>
          <ProfileDataProvider>
            <LocationProvider>
              <AppShell>
                <AppRoutes />
              </AppShell>
            </LocationProvider>
          </ProfileDataProvider>
        </PrinterDiscoveryProvider>
      </PrinterInventoryProvider>
    </ServerStatusProvider>
  );
}
