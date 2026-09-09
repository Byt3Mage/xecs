## insert_bundle(id, bundle)

1. Look up r = ids[id].
2. Compute dst_table from `r.table + bundle.ids()`.
3. Pre-barrier, entity still whole in src: for each cid in `src_sig \ dst_sig` → on_remove(cid).
4. Allocate dst_row in dst_table.
5. Walk dst columns. For each cid in dst_sig:
    - in bundle → drop the src value if cid is in src_sig, then write the payload into dst_row
    - not in bundle → move from src row
6. Drop leftovers. For each cid in src_sig \ dst_sig → drop the src value.
7. Vacate the src row; repoint the displaced id; set ids[id] = (dst_table, dst_row).
8. Post-barrier, entity whole in dst:
    - for each cid in bundle \ src_sig → on_add(cid)
    - for each cid in bundle → on_insert(cid)

remove(id, cid): 1, 2, 3, 4–7 with an empty bundle, no 8.

delete_id(id): `on_remove` for every cid in src_sig, drop the row, free the id.

Same-table case (`dst_table == r.table`): 1, 2, no 3, then for each field drop the old value and write the payload, then 8.

_inline_
