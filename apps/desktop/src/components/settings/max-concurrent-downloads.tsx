import { Label } from "@deadlock-mods/ui/components/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@deadlock-mods/ui/components/select";
import { invoke } from "@tauri-apps/api/core";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import logger from "@/lib/logger";
import { usePersistedStore } from "@/lib/store";

const CONCURRENT_DOWNLOAD_OPTIONS = [
  { value: "1", label: "1" },
  { value: "2", label: "2" },
  { value: "3", label: "3" },
  { value: "4", label: "4" },
  { value: "5", label: "5" },
  { value: "8", label: "8" },
  { value: "10", label: "10" },
];

export const MaxConcurrentDownloads = () => {
  const { t } = useTranslation();
  const maxConcurrentDownloads = usePersistedStore(
    (state) => state.maxConcurrentDownloads,
  );
  const setMaxConcurrentDownloads = usePersistedStore(
    (state) => state.setMaxConcurrentDownloads,
  );

  // Sync the current value with the Rust backend whenever it changes.
  useEffect(() => {
    invoke("set_max_concurrent_downloads", {
      maxConcurrent: maxConcurrentDownloads,
    }).catch((error) => {
      logger
        .withError(error instanceof Error ? error : new Error(String(error)))
        .error("Failed to sync max concurrent downloads with backend");
    });
  }, [maxConcurrentDownloads]);

  const handleChange = (value: string) => {
    const count = Number.parseInt(value, 10);
    if (!Number.isNaN(count)) {
      setMaxConcurrentDownloads(count);
    }
  };

  return (
    <div className='flex items-center justify-between'>
      <div className='space-y-1'>
        <Label>{t("settings.maxConcurrentDownloads")}</Label>
        <p className='text-muted-foreground text-sm'>
          {t("settings.maxConcurrentDownloadsDescription")}
        </p>
      </div>
      <Select
        onValueChange={handleChange}
        value={String(maxConcurrentDownloads)}>
        <SelectTrigger className='w-24'>
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {CONCURRENT_DOWNLOAD_OPTIONS.map((option) => (
            <SelectItem key={option.value} value={option.value}>
              {option.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
};
