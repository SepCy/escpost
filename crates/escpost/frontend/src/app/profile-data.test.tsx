import { afterEach, describe, expect, jest, test } from "bun:test";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/preact";
import { ProfileDataProvider, useProfileData } from "./profile-data";

const originalFetch = globalThis.fetch;

function Probe() {
  const { ensureProfiles, profiles } = useProfileData();
  return <>
    <button type="button" onClick={() => void ensureProfiles()}>Profiles</button>
    <p>{`${profiles.phase}:${profiles.data?.profiles.length ?? "none"}`}</p>
  </>;
}

afterEach(() => { cleanup(); globalThis.fetch = originalFetch; });

describe("ProfileDataProvider", () => {
  test("loads the profile catalog only when a consumer asks for it", async () => {
    const fetch = jest.fn((_input: RequestInfo | URL) => Promise.resolve(new Response(JSON.stringify({ profiles: [] }), { headers: { "content-type": "application/json" } })));
    globalThis.fetch = fetch as unknown as typeof globalThis.fetch;
    render(<ProfileDataProvider><Probe /></ProfileDataProvider>);

    expect(fetch).not.toHaveBeenCalled();
    await act(async () => {
      fireEvent.click(screen.getByRole("button", { name: "Profiles" }));
      for (let turn = 0; turn < 6; turn += 1) await Promise.resolve();
    });

    expect(fetch.mock.calls.map(([input]) => String(input))).toEqual(["/api/profiles/list"]);
    expect(screen.getByText("ready:0")).toBeTruthy();
  });
});
