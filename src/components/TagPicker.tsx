// 标签选择/自定义组件（F-05）：多选 + 即时创建新标签
import React, { useState } from "react";
import { Autocomplete, Chip, TextField } from "@mui/material";
import type { Tag } from "../types";

interface TagPickerProps {
  allTags: Tag[];
  selectedIds: number[];
  onChange: (ids: number[]) => void;
  onCreate: (name: string) => Promise<number>; // 返回新标签 ID
}

export default function TagPicker({
  allTags,
  selectedIds,
  onChange,
  onCreate,
}: TagPickerProps): React.JSX.Element {
  const [inputValue, setInputValue] = useState("");
  const [creating, setCreating] = useState(false);

  const selected = allTags.filter((t) => selectedIds.includes(t.id));

  return (
    <Autocomplete
      multiple
      size="small"
      options={allTags}
      getOptionLabel={(o) => o.name}
      value={selected}
      inputValue={inputValue}
      onInputChange={(_, v) => setInputValue(v)}
      onChange={async (_, values) => {
        onChange(values.map((v) => v.id));
      }}
      noOptionsText={
        inputValue.trim() ? (
          <Chip
            size="small"
            color="primary"
            disabled={creating}
            label={`创建标签「${inputValue.trim()}」`}
            onClick={async () => {
              setCreating(true);
              try {
                const id = await onCreate(inputValue.trim());
                onChange([...selectedIds, id]);
                setInputValue("");
              } finally {
                setCreating(false);
              }
            }}
          />
        ) : (
          "输入以创建标签"
        )
      }
      renderTags={(value, getTagProps) =>
        value.map((tag, index) => (
          <Chip
            size="small"
            label={tag.name}
            {...getTagProps({ index })}
            key={tag.id}
            style={tag.color ? { backgroundColor: tag.color, color: "#fff" } : undefined}
          />
        ))
      }
      renderInput={(params) => (
        <TextField {...params} label="标签" placeholder="选择或输入新标签" />
      )}
    />
  );
}
