import { Card, CardContent, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { useAppInfo } from "./queries";

export function AppInfoCard() {
  const { data, isPending, error } = useAppInfo();

  return (
    <Card className="w-80">
      <CardHeader>
        <CardTitle>{data?.name ?? "Orbis Scriptor"}</CardTitle>
        <CardDescription>Local-first worldbuilding</CardDescription>
      </CardHeader>
      <CardContent className="text-sm text-muted-foreground">
        {isPending && <p>Loading…</p>}
        {error && <p className="text-destructive">Could not load app info: {error.message}</p>}
        {data && <p>Version {data.version}</p>}
      </CardContent>
    </Card>
  );
}
