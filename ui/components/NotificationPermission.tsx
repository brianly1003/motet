"use client";
import { useEffect, useState } from "react";
import * as ipc from "@/lib/ipc";
import { strings as s } from "@/lib/strings";

export default function NotificationPermission() {
  const [allowed, setAllowed] = useState(true);
  const [message, setMessage] = useState(s.permissionBody);
  const [pending, setPending] = useState(false);
  useEffect(() => {
    if (!ipc.desktopAvailable()) return;
    let active = true;
    ipc.notificationPermission().then(value => { if (active) setAllowed(value); })
      .catch(error => { if (active) { setAllowed(false); setMessage(String(error)); } });
    return () => { active = false; };
  }, []);
  async function request() {
    setPending(true);
    try { const value = await ipc.notificationPermission(true); setAllowed(value); if (!value) setMessage(s.notificationsDenied); }
    catch (error) { setMessage(String(error)); }
    finally { setPending(false); }
  }
  return !allowed && <div className="banner" role="status"><p>{message}</p><button className="quiet" disabled={pending} onClick={request}>{s.allowNotifications}</button></div>;
}
