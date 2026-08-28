import { createContext } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";
import { openDiscoveryStream } from "../api/discovery-stream";
import type { DiscoveryQuery, UsbDiscoveryFailure } from "../api/discovery-stream";
import type { AddPrinterBody, DiscoveredPrinter } from "../api/types";
import { useReportDiscoveredPrinters } from "./printer-inventory-data";

type ScanPhase = "idle" | "running" | "done" | "stopped" | "error";

export type ScanState = {
  phase: ScanPhase;
  completed: number;
  total: number;
  printers: DiscoveredPrinter[];
  failures: UsbDiscoveryFailure[];
  error: string | null;
};

type PrinterDiscoveryData = {
  scan: ScanState;
  scanQuery: DiscoveryQuery;
  startScan: (query: DiscoveryQuery) => void;
  cancelScan: () => void;
  markScanResultConfigured: (name: string, connection: AddPrinterBody["connection"]) => void;
};

const initialScanQuery: DiscoveryQuery = { usb: true, network: true, subnets: [] };
const initialScan: ScanState = { phase: "idle", completed: 0, total: 0, printers: [], failures: [], error: null };
const PrinterDiscoveryContext = createContext<PrinterDiscoveryData | null>(null);

function registeredAs(discovered: DiscoveredPrinter, connection: AddPrinterBody["connection"]) {
  const found = discovered.connection;
  if (connection.type === "network") {
    return found.type === "network" && found.host === connection.host && found.port === connection.port;
  }
  return found.type === "usb"
    && found.vendor_id === connection.vendor_id
    && found.product_id === connection.product_id
    && found.interface_number === connection.interface_number
    && found.out_endpoints.includes(connection.out_endpoint)
    && (connection.serial_number === null || found.serial_number === connection.serial_number);
}

export function PrinterDiscoveryProvider({ children }: { children: preact.ComponentChildren }) {
  const reportDiscoveredPrinters = useReportDiscoveredPrinters();
  const [scan, setScan] = useState<ScanState>(initialScan);
  const [scanQuery, setScanQuery] = useState<DiscoveryQuery>(initialScanQuery);
  const scanCloser = useRef<(() => void) | null>(null);

  const closeScan = useCallback(() => {
    scanCloser.current?.();
    scanCloser.current = null;
  }, []);

  const handleDiscoveredPrinter = useCallback((printer: DiscoveredPrinter) => {
    setScan((current) => ({ ...current, printers: [...current.printers, printer] }));
    if (printer.configured_names.length > 0) reportDiscoveredPrinters(printer.configured_names);
  }, [reportDiscoveredPrinters]);

  const startScan = useCallback((query: DiscoveryQuery) => {
    closeScan();
    setScan({ ...initialScan, phase: "running" });
    setScanQuery(query);
    scanCloser.current = openDiscoveryStream(query, {
      onPrepared: (prepared) => setScan((current) => ({ ...current, total: prepared.total_probes })),
      onPrinter: handleDiscoveredPrinter,
      onProgress: (progress) => setScan((current) => ({ ...current, completed: progress.completed, total: progress.total })),
      onUsbFailure: (failure) => setScan((current) => ({ ...current, failures: [...current.failures, failure] })),
      onCompleted: () => { scanCloser.current = null; setScan((current) => ({ ...current, phase: "done" })); },
      onError: (error) => { scanCloser.current = null; setScan((current) => ({ ...current, phase: "error", error })); },
    });
  }, [closeScan, handleDiscoveredPrinter]);

  const cancelScan = useCallback(() => {
    closeScan();
    setScan((current) => current.phase === "running" ? { ...current, phase: "stopped" } : current);
  }, [closeScan]);

  const markScanResultConfigured = useCallback((name: string, connection: AddPrinterBody["connection"]) => {
    setScan((current) => {
      const index = current.printers.findIndex((printer) => registeredAs(printer, connection) && !printer.configured_names.includes(name));
      if (index === -1) return current;
      const printers = [...current.printers];
      const found = printers[index]!;
      printers[index] = { ...found, configured_names: [...found.configured_names, name] };
      return { ...current, printers };
    });
  }, []);

  useEffect(() => () => { closeScan(); }, [closeScan]);

  return <PrinterDiscoveryContext.Provider value={{ scan, scanQuery, startScan, cancelScan, markScanResultConfigured }}>{children}</PrinterDiscoveryContext.Provider>;
}

export function usePrinterDiscovery() {
  const data = useContext(PrinterDiscoveryContext);
  if (!data) throw new Error("usePrinterDiscovery must be used within PrinterDiscoveryProvider.");
  return data;
}
