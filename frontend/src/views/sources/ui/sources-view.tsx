"use client";

import { Badge, Skeleton, Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@evinvest/uikit";
import { useState } from "react";

import { managesSources, useMe } from "@/entities/session";
import { type AddedSource, fetchSources } from "@/entities/source";
import { AddSourceForm, RevokeButton, SecretDialog } from "@/features/manage-sources";
import { useLocale, useT } from "@/shared/i18n";
import { formatDateTime } from "@/shared/lib/format";
import { useResource } from "@/shared/lib/use-resource";
import { EmptyState } from "@/shared/ui/empty-state";
import { ErrorState } from "@/shared/ui/error-state";
import { PageHeader } from "@/shared/ui/page-header";

/** Admin only (spec §5.4): the signing keys the sources ingest with. */
export function SourcesView() {
  const t = useT();
  const locale = useLocale();
  const { role } = useMe();
  const sources = useResource("sources", fetchSources);
  const [added, setAdded] = useState<AddedSource | null>(null);

  if (!managesSources(role)) return <div className="p-4 md:p-6"><ErrorState failure={{ kind: "forbidden", message: "" }} /></div>;

  return (
    <div className="flex flex-col gap-4 p-4 md:p-6">
      <PageHeader title={t("sources.title")} />
      <AddSourceForm
        onAdded={(a) => {
          setAdded(a);
          sources.reload();
        }}
      />
      {sources.status === "loading" && <Skeleton className="h-40 w-full" />}
      {sources.status === "error" && <ErrorState failure={sources.failure} onRetry={sources.reload} />}
      {sources.status === "ok" && sources.data.length === 0 && <EmptyState title={t("sources.empty")} description={t("sources.empty.body")} />}
      {sources.status === "ok" && sources.data.length > 0 && (
        <div className="overflow-x-auto">
          <Table>
            <TableHeader>
              <TableRow>
                <TableHead>{t("sources.keyId")}</TableHead>
                <TableHead>{t("sources.kind")}</TableHead>
                <TableHead>{t("sources.brands")}</TableHead>
                <TableHead>{t("sources.created")}</TableHead>
                <TableHead />
              </TableRow>
            </TableHeader>
            <TableBody>
              {sources.data.map((s) => (
                <TableRow key={s.key_id}>
                  <TableCell className="font-mono text-sm">{s.key_id}</TableCell>
                  <TableCell>{s.kind}</TableCell>
                  <TableCell>{s.brands.join(", ")}</TableCell>
                  <TableCell className="tabular-nums text-ink-soft">{formatDateTime(s.created_at, locale)}</TableCell>
                  <TableCell className="text-right">
                    {s.revoked_at ? (
                      <Badge variant="outline">{t("sources.revoked", { at: formatDateTime(s.revoked_at, locale) })}</Badge>
                    ) : (
                      <RevokeButton keyId={s.key_id} onRevoked={sources.reload} />
                    )}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
      <SecretDialog added={added} onClose={() => setAdded(null)} />
    </div>
  );
}
