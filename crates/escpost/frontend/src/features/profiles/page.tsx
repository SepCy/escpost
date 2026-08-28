import { useProfileData } from "../../app/profile-data";
import { useEffect } from "preact/hooks";
import { ProfileList } from "./profile-list";

export function ProfilesPage() {
  const { ensureProfiles } = useProfileData();
  useEffect(() => {
    void ensureProfiles();
  }, [ensureProfiles]);
  return (
    <section aria-labelledby="profiles-heading" class="space-y-6">
      <h1 id="profiles-heading" class="sr-only">Profiles</h1>
      <ProfileList />
    </section>
  );
}
