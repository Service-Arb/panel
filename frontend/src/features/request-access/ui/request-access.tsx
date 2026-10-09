"use client";

import { Avatar, AvatarFallback, Button, Card, CardContent, CardDescription, CardHeader, CardTitle, Skeleton, cn } from "@evinvest/uikit";
import { Check } from "lucide-react";
import { type ReactNode, useEffect, useState } from "react";

import { type AccessRequest, type Caller, fetchMe } from "@/entities/session";
import { ApiError, switchAccountPath } from "@/shared/api";
import { type MessageKey, useLocale, useT } from "@/shared/i18n";
import { useResource } from "@/shared/lib/use-resource";
import { ErrorState } from "@/shared/ui/error-state";
import { failureText } from "@/shared/ui/failure-text";
import { useButtonSize } from "@/shared/ui/touch";

import { fetchMyRequests, requestAccess } from "../api/requests";
import { type Need, PLAYBOOK_NEED, holds } from "../model/access";

const RECHECK_MS = 30_000;

/**
 * Asking the admins for `need`, then waiting on `/me` until it is held: then on to
 * `continueTo` (a path already checked by `safeContinue`), or "you're in" without one.
 */
export function RequestAccess({ need, continueTo }: { need: Need; continueTo: string | null }) {
  const me = useResource("me", fetchMe);
  const mine = useResource("access.mine", fetchMyRequests);
  const [sent, setSent] = useState<AccessRequest | null>(null);
  const held = me.status === "ok" && holds(me.data.permissions, need);
  const pending = sent ?? (mine.status === "ok" ? (mine.data.find((r) => r.need === need) ?? null) : null);
  const waiting = pending !== null && !held;
  const reloadMe = me.reload;

  useEffect(() => {
    if (held && continueTo !== null) window.location.assign(continueTo);
  }, [held, continueTo]);
  useEffect(() => {
    if (!waiting) return;
    const id = setInterval(reloadMe, RECHECK_MS);
    return () => clearInterval(id);
  }, [waiting, reloadMe]);

  if (me.status === "error" && me.failure.kind !== "unauthenticated") return <Frame><ErrorState failure={me.failure} onRetry={me.reload} /></Frame>;
  if (mine.status === "error" && mine.failure.kind !== "unauthenticated") return <Frame><ErrorState failure={mine.failure} onRetry={mine.reload} /></Frame>;
  // A 401 has already sent the browser to sign in; a need held with somewhere to go is on its way there.
  if (me.status !== "ok" || mine.status !== "ok" || (held && continueTo !== null)) return <Frame><Skeleton className="h-96 w-full" /></Frame>;
  if (held) return <Granted />;
  return <Asking need={need} continueTo={continueTo} me={me.data} pending={pending} sentHere={sent !== null} onSent={setSent} onHeld={reloadMe} />;
}

function Frame({ children }: { children: ReactNode }) {
  return (
    <main className="flex min-h-svh items-center justify-center p-4">
      <div className="w-full max-w-md">{children}</div>
    </main>
  );
}

function Granted() {
  const t = useT();
  const button = useButtonSize();
  return (
    <Frame>
      <Card>
        <CardHeader className="text-center">
          <Marks client={false} />
          <CardTitle>{t("access.in.title")}</CardTitle>
          <CardDescription>{t("access.in.body")}</CardDescription>
        </CardHeader>
        <CardContent>
          <Button asChild className="w-full" size={button("lg")}>
            {/* A full load: a shell that saw the access gone reads it again only from scratch. */}
            {/* eslint-disable-next-line @next/next/no-html-link-for-pages */}
            <a href="/">{t("access.in.open")}</a>
          </Button>
        </CardContent>
      </Card>
    </Frame>
  );
}

function Asking({
  need,
  continueTo,
  me,
  pending,
  sentHere,
  onSent,
  onHeld,
}: {
  need: Need;
  continueTo: string | null;
  me: Caller;
  pending: AccessRequest | null;
  sentHere: boolean;
  onSent: (r: AccessRequest) => void;
  onHeld: () => void;
}) {
  const t = useT();
  const locale = useLocale();
  const button = useButtonSize();
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);
  const playbook = need === PLAYBOOK_NEED;

  const ask = () => {
    setBusy(true);
    setFailure(null);
    requestAccess(need).then(
      (r) => {
        setBusy(false);
        onSent(r);
      },
      (e: unknown) => {
        setBusy(false);
        if (e instanceof ApiError && e.failure.kind === "conflict") return onHeld(); // held already: /me says so on its next read
        setFailure(e instanceof ApiError ? failureText(e.failure, t) : t("state.error", { detail: String(e) }));
      },
    );
  };

  const at = pending && new Intl.DateTimeFormat(locale, { hour: "2-digit", minute: "2-digit" }).format(new Date(pending.requested_at));
  const [title, body]: [MessageKey, MessageKey] = pending
    ? ["access.sent.title", "access.sent.body"]
    : [playbook ? "access.playbook.title" : "access.title", "access.body"];
  const here = window.location.pathname + window.location.search;

  return (
    <Frame>
      <Card>
        <CardHeader className="text-center">
          <Marks client={continueTo !== null} />
          <CardTitle>{t(title)}</CardTitle>
          <CardDescription>{t(body)}</CardDescription>
        </CardHeader>
        <CardContent className="flex flex-col gap-5">
          <div className="flex items-center gap-3 rounded-md border border-border p-3">
            <Avatar>
              <AvatarFallback>{(me.preferred_name || me.email).charAt(0).toUpperCase()}</AvatarFallback>
            </Avatar>
            <div className="min-w-0 text-sm">
              <p className="text-ink-soft">{t("access.account")}</p>
              <p className="truncate text-ink">{me.email}</p>
            </div>
          </div>
          <ol className="flex flex-col gap-3">
            <Step n={1} done={pending !== null} title={t("access.step.request")}>
              {at ? t(sentHere ? "access.step.request.sentNow" : "access.step.request.sent", { time: at }) : t("access.step.request.hint")}
            </Step>
            <Step n={2} done={false} title={t("access.step.wait")}>
              {t("access.step.wait.hint")}
            </Step>
            {continueTo !== null && (
              <Step n={3} done={false} title={t("access.step.back")}>
                {t("access.step.back.hint")}
              </Step>
            )}
          </ol>
          <div className="flex flex-col gap-2">
            {pending ? (
              <p role="status" aria-live="polite" className="flex items-center justify-center gap-2 py-2 text-sm text-ink-soft">
                <span aria-hidden className="size-2 shrink-0 rounded-full bg-accent-warn motion-safe:animate-pulse" />
                {t("access.waiting")}
              </p>
            ) : (
              <Button className="w-full" size={button("lg")} onClick={ask} disabled={busy}>
                {t("access.request")}
              </Button>
            )}
            {failure && <p className="text-sm text-accent-error">{failure}</p>}
            <Button asChild variant="outline" className="w-full" size={button("lg")}>
              <a href={switchAccountPath(here)}>{t("access.switch")}</a>
            </Button>
            {continueTo !== null && (
              <a className="py-2 text-center text-sm text-ink-soft hover:underline" href={`${continueTo}${continueTo.includes("?") ? "&" : "?"}decision=deny`}>
                {t("access.cancel")}
              </a>
            )}
          </div>
        </CardContent>
      </Card>
    </Frame>
  );
}

function Step({ n, done, title, children }: { n: number; done: boolean; title: string; children: ReactNode }) {
  return (
    <li className="flex gap-3">
      <span
        aria-hidden
        className={cn(
          "flex size-6 shrink-0 items-center justify-center rounded-full text-xs font-medium",
          done ? "bg-positive text-background" : "bg-muted text-ink-soft",
        )}
      >
        {done ? <Check className="size-3.5" /> : n}
      </span>
      <div className="text-sm">
        <p className="font-medium text-ink">{title}</p>
        <p className="text-ink-soft">{children}</p>
      </div>
    </li>
  );
}

/** The client's mark • • • Service-Arb's; Service-Arb's alone when no client is waiting. */
function Marks({ client }: { client: boolean }) {
  const mark = "flex size-10 items-center justify-center rounded-lg text-sm font-semibold";
  return (
    <div aria-hidden className="mb-2 flex items-center justify-center gap-3">
      {client && (
        <>
          <span className={cn(mark, "bg-muted text-ink")}>C</span>
          <span className="flex gap-1">
            {[0, 1, 2].map((i) => (
              <span key={i} className="size-1 rounded-full bg-ink-soft" />
            ))}
          </span>
        </>
      )}
      <span className={cn(mark, "bg-primary text-background")}>SA</span>
    </div>
  );
}
