import { afterEach, describe, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/preact";
import { PrinterDiscoveryProvider, usePrinterDiscovery } from "./printer-discovery-data";
import { PrinterInventoryProvider } from "./printer-inventory-data";

class FakeEventSource {
  static instances: FakeEventSource[] = [];
  static order = 0;
  closed = false;
  readonly constructedAt: number;
  closedAt: number | null = null;
  private readonly listeners = new Map<string, ((event: Event) => void)[]>();
  constructor(readonly url: string) {
    this.constructedAt = FakeEventSource.order++;
    FakeEventSource.instances.push(this);
  }
  addEventListener(name: string, handler: (event: Event) => void) { this.listeners.set(name, [...(this.listeners.get(name) ?? []), handler]); }
  close() { this.closed = true; this.closedAt = FakeEventSource.order++; }
  emit(name: string, data: unknown) {
    for (const handler of this.listeners.get(name) ?? []) handler(new MessageEvent(name, { data: JSON.stringify(data) }));
  }
  static discoverySources() { return FakeEventSource.instances.filter((source) => source.url.startsWith("/api/printers/discover")); }
}

const originalEventSource = globalThis.EventSource;
const query = { usb: true, network: true, subnets: [], port: 9100, timeoutMs: 1000 };

function Probe() {
  const { startScan, scan, markScanResultConfigured } = usePrinterDiscovery();
  return <>
    <button type="button" onClick={() => startScan(query)}>Scan</button>
    <button type="button" onClick={() => markScanResultConfigured("Kitchen", { type: "network", host: "10.0.0.8", port: 9100 })}>Configure network</button>
    <button type="button" onClick={() => markScanResultConfigured("USB one", { type: "usb", vendor_id: 1046, product_id: 20497, serial_number: null, interface_number: 0, out_endpoint: 1, in_endpoint: null })}>Configure USB</button>
    <p>{`${scan.phase}:${scan.printers.length}:${scan.failures.map((failure) => failure.product_id).join(",")}`}</p>
    <p data-testid="configured">{JSON.stringify(scan.printers.map((printer) => printer.configured_names))}</p>
  </>;
}

function renderProvider() {
  FakeEventSource.instances = [];
  FakeEventSource.order = 0;
  globalThis.EventSource = FakeEventSource as unknown as typeof EventSource;
  return render(<PrinterInventoryProvider><PrinterDiscoveryProvider><Probe /></PrinterDiscoveryProvider></PrinterInventoryProvider>);
}

afterEach(() => { cleanup(); globalThis.EventSource = originalEventSource; });

describe("PrinterDiscoveryProvider", () => {
  test("closes a discovery source before constructing its replacement and preserves ordered USB failures", () => {
    renderProvider();
    act(() => { fireEvent.click(screen.getByRole("button", { name: "Scan" })); });
    expect(FakeEventSource.discoverySources()).toHaveLength(1);
    const first = FakeEventSource.discoverySources()[0]!;
    act(() => first.emit("usb_failure", { vendor_id: 1046, product_id: 2, stage: "open_device", reason: "denied", permission_denied: true, can_grant_usb_permissions: true }));
    act(() => first.emit("usb_failure", { vendor_id: 1046, product_id: 3, stage: "open_device", reason: "denied", permission_denied: true, can_grant_usb_permissions: true }));
    expect(screen.getByText("running:0:2,3")).toBeTruthy();

    act(() => { fireEvent.click(screen.getByRole("button", { name: "Scan" })); });
    const second = FakeEventSource.discoverySources()[1]!;
    expect(first.closed).toBe(true);
    expect(first.closedAt).not.toBeNull();
    expect(first.closedAt as number).toBeLessThan(second.constructedAt);
  });

  test("marks only matching network and one ambiguous USB discovery result configured", () => {
    renderProvider();
    act(() => { fireEvent.click(screen.getByRole("button", { name: "Scan" })); });
    const source = FakeEventSource.discoverySources()[0]!;
    const usb = (host: string) => ({
      transport: "usb", configured_names: [], configured_profile: null,
      connection: { type: "usb", vendor_id: 1046, product_id: 20497, bus: "003", address: 7, manufacturer: null, product: host, serial_number: null, interface_number: 0, out_endpoints: [1], in_endpoints: [] },
    });
    act(() => source.emit("printer", { transport: "network", configured_names: [], configured_profile: null, connection: { type: "network", host: "10.0.0.8", port: 9100 } }));
    act(() => source.emit("printer", { transport: "network", configured_names: [], configured_profile: null, connection: { type: "network", host: "10.0.0.9", port: 9100 } }));
    act(() => source.emit("printer", usb("first")));
    act(() => source.emit("printer", usb("second")));

    fireEvent.click(screen.getByRole("button", { name: "Configure network" }));
    fireEvent.click(screen.getByRole("button", { name: "Configure USB" }));
    expect(screen.getByTestId("configured").textContent).toBe("[[\"Kitchen\"],[],[\"USB one\"],[]]");
  });
});
