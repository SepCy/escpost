import { createContext } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";
import { openPrinterInventoryStream } from "../api/printer-inventory-stream";
import type { PrintersResponse } from "../api/types";

export type PrinterFlashes = Record<string, "found" | "lost">;

export type PrinterInventoryResource =
  | { phase: "checking"; snapshot: null; error: null; printerFlashes: PrinterFlashes }
  | { phase: "ready"; snapshot: PrintersResponse; error: null; printerFlashes: PrinterFlashes }
  | { phase: "disconnected"; snapshot: PrintersResponse | null; error: Error; printerFlashes: PrinterFlashes };

type PrinterInventoryData = {
  resource: PrinterInventoryResource;
  reportDiscoveredPrinters: (names: string[]) => void;
};

const PrinterInventoryContext = createContext<PrinterInventoryData | null>(null);
const FLASH_DURATION = 1_200;
const RETRY_DELAY_MS = 2_000;

function nextFlashes(previous: PrintersResponse | null, next: PrintersResponse, current: PrinterFlashes): PrinterFlashes {
  if (!previous) return current;
  const previousAvailability = new Map(previous.printers.map((printer) => [printer.name, printer.availability]));
  const flashes = { ...current };
  for (const printer of next.printers) {
    const before = previousAvailability.get(printer.name);
    if (before === undefined || (before === "unavailable" && printer.availability === "connected")) {
      flashes[printer.name] = "found";
    } else if (before === "connected" && printer.availability === "unavailable") {
      flashes[printer.name] = "lost";
    }
  }
  return flashes;
}

export function PrinterInventoryProvider({ children, retryDelayMs = RETRY_DELAY_MS }: {
  children: preact.ComponentChildren;
  retryDelayMs?: number;
}) {
  const [resource, setResource] = useState<PrinterInventoryResource>({
    phase: "checking", snapshot: null, error: null, printerFlashes: {},
  });
  const [attempt, setAttempt] = useState(0);
  const timeouts = useRef<Map<string, ReturnType<typeof setTimeout>>>(new Map());

  const clearFlashAfterDelay = useCallback((name: string, flash: "found" | "lost") => {
    const pending = timeouts.current.get(name);
    if (pending !== undefined) clearTimeout(pending);
    timeouts.current.set(name, setTimeout(() => {
      timeouts.current.delete(name);
      setResource((latest) => {
        if (latest.printerFlashes[name] !== flash) return latest;
        const nextFlashes = { ...latest.printerFlashes };
        delete nextFlashes[name];
        return { ...latest, printerFlashes: nextFlashes } as PrinterInventoryResource;
      });
    }, FLASH_DURATION));
  }, []);

  const reportDiscoveredPrinters = useCallback((names: string[]) => {
    const discovered = new Set(names);
    setResource((current) => {
      if (!current.snapshot) return current;
      const matched = current.snapshot.printers.filter((printer) => discovered.has(printer.name));
      if (matched.length === 0) return current;
      const printerFlashes = { ...current.printerFlashes };
      for (const printer of matched) {
        printerFlashes[printer.name] = "found";
        clearFlashAfterDelay(printer.name, "found");
      }
      return {
        ...current,
        snapshot: {
          ...current.snapshot,
          printers: current.snapshot.printers.map((printer) => (
            discovered.has(printer.name) ? { ...printer, availability: "connected" as const } : printer
          )),
        },
        printerFlashes,
      } as PrinterInventoryResource;
    });
  }, [clearFlashAfterDelay]);

  useEffect(() => {
    let retry: ReturnType<typeof setTimeout> | undefined;
    const close = openPrinterInventoryStream({
      onSnapshot: (snapshot) => {
        setResource((current) => {
          const flashes = nextFlashes(current.snapshot, snapshot, current.printerFlashes);
          for (const [name, flash] of Object.entries(flashes)) {
            if (current.printerFlashes[name] === flash) continue;
            clearFlashAfterDelay(name, flash);
          }
          return { phase: "ready", snapshot, error: null, printerFlashes: flashes };
        });
      },
      onError: (error) => {
        setResource((current) => ({
          phase: "disconnected", snapshot: current.snapshot, error, printerFlashes: current.printerFlashes,
        }));
        retry ??= setTimeout(() => setAttempt((current) => current + 1), retryDelayMs);
      },
    });
    return () => {
      clearTimeout(retry);
      close();
    };
  }, [attempt, clearFlashAfterDelay, retryDelayMs]);

  useEffect(() => () => {
    for (const timeout of timeouts.current.values()) clearTimeout(timeout);
    timeouts.current.clear();
  }, []);

  return (
    <PrinterInventoryContext.Provider value={{ resource, reportDiscoveredPrinters }}>
      {children}
    </PrinterInventoryContext.Provider>
  );
}

function usePrinterInventoryData() {
  const data = useContext(PrinterInventoryContext);
  if (!data) throw new Error("Printer inventory data must be used within PrinterInventoryProvider.");
  return data;
}

export function usePrinterInventory(): PrinterInventoryResource {
  return usePrinterInventoryData().resource;
}

export function useReportDiscoveredPrinters() {
  return usePrinterInventoryData().reportDiscoveredPrinters;
}
