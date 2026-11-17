// 冲突合并界面（F-16/AC-8）：双版本 Diff + 裁决
import React, { useEffect, useState } from "react";
import { Box, CircularProgress, Paper, Stack, Typography } from "@mui/material";
import { attributionApi } from "../api/attribution";
import { useAppStore } from "../stores/appStore";
import ConflictDiff from "../components/ConflictDiff";
import type { Conflict } from "../types";

export default function ConflictMergePage(): React.JSX.Element {
  const notify = useAppStore((s) => s.notify);
  const [conflicts, setConflicts] = useState<Conflict[] | null>(null);
  const [resolving, setResolving] = useState(false);

  const load = async () => {
    try {
      setConflicts(await attributionApi.conflictList());
    } catch {
      setConflicts([]);
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const resolve = async (
    conflictId: number,
    choice: "local" | "cloud" | "merged",
    mergedLabel?: string,
    mergedDetail?: string
  ) => {
    setResolving(true);
    try {
      await attributionApi.conflictResolve(conflictId, choice, mergedLabel, mergedDetail);
      notify({ severity: "success", message: "冲突已裁决，双版本记录保留" });
      await load();
    } catch {
      /* client 已弹错 */
    } finally {
      setResolving(false);
    }
  };

  if (conflicts === null) {
    return (
      <Box sx={{ display: "flex", justifyContent: "center", mt: 8 }}>
        <CircularProgress />
      </Box>
    );
  }

  return (
    <Box>
      <Typography variant="h5" sx={{ mb: 1, fontWeight: 700 }}>
        冲突合并
      </Typography>
      <Typography variant="body2" color="text.secondary" sx={{ mb: 2 }}>
        离线本地初判与云端深判不一致的归因会出现在这里。裁决后双版本永久保留，可随时追溯。
      </Typography>
      {conflicts.length === 0 ? (
        <Paper sx={{ p: 4, textAlign: "center" }}>
          <Typography color="text.secondary">没有待合并的冲突，一切一致。</Typography>
        </Paper>
      ) : (
        <Stack spacing={2}>
          {conflicts.map((c) => (
            <ConflictDiff key={c.id} conflict={c} resolving={resolving} onResolve={resolve} />
          ))}
        </Stack>
      )}
    </Box>
  );
}
