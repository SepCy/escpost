import { createContext } from "preact";
import { useCallback, useContext, useEffect, useRef, useState } from "preact/hooks";
import { getProfiles } from "../api/client";
import type { ProfilesResponse } from "../api/types";

type ResourcePhase = "loading" | "ready" | "refreshing" | "error";
export type ProfileResource = { data: ProfilesResponse | null; error: Error | null; phase: ResourcePhase };

type ProfileData = {
  profiles: ProfileResource;
  ensureProfiles: () => Promise<void>;
  refreshProfiles: () => Promise<void>;
};

const ProfileDataContext = createContext<ProfileData | null>(null);
const initialProfiles: ProfileResource = { data: null, error: null, phase: "loading" };

export function ProfileDataProvider({ children }: { children: preact.ComponentChildren }) {
  const [profiles, setProfiles] = useState<ProfileResource>(initialProfiles);
  const profileData = useRef<ProfilesResponse | null>(null);
  const profileRequest = useRef<Promise<void> | null>(null);
  const profileAbort = useRef<AbortController | null>(null);

  const refreshProfiles = useCallback(async () => {
    if (profileRequest.current) return profileRequest.current;
    const controller = new AbortController();
    profileAbort.current = controller;
    setProfiles((current) => ({ data: current.data, error: null, phase: current.data ? "refreshing" : "loading" }));
    const request = getProfiles(controller.signal)
      .then((data) => { profileData.current = data; setProfiles({ data, error: null, phase: "ready" }); })
      .catch((error: unknown) => {
        if (controller.signal.aborted) return;
        setProfiles({ data: profileData.current, error: error instanceof Error ? error : new Error("Unable to load profile catalog."), phase: profileData.current ? "ready" : "error" });
      })
      .finally(() => { if (profileAbort.current === controller) profileAbort.current = null; profileRequest.current = null; });
    profileRequest.current = request;
    return request;
  }, []);

  const ensureProfiles = useCallback(async () => {
    if (!profileData.current) return refreshProfiles();
  }, [refreshProfiles]);

  useEffect(() => () => { profileAbort.current?.abort(); }, []);

  return <ProfileDataContext.Provider value={{ profiles, ensureProfiles, refreshProfiles }}>{children}</ProfileDataContext.Provider>;
}

export function useProfileData() {
  const data = useContext(ProfileDataContext);
  if (!data) throw new Error("useProfileData must be used within ProfileDataProvider.");
  return data;
}
