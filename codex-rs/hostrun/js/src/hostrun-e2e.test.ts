import { describe, expect, it } from "vitest";
import { HostrunSession, type HostrunApprovalRequest } from "./index.js";

describe("Hostrun end-to-end readable automation", () => {
  it("stores files in ctx, filters with containing, and deletes approved fake remotes", async () => {
    const approvals: HostrunApprovalRequest[] = [];
    const deletedTargets: string[] = [];
    const session = await HostrunSession.create({
      approve: (approval) => {
        approvals.push(approval);
        return { type: "approve" };
      },
      capabilities: {
        "rclone.lsf": {
          describe: (args) => ({
            id: "approval-lsf",
            tool: "rclone.lsf",
            summary: `List ${args.path}`,
            args,
          }),
          invoke: () => [
            "publisher-a/codex-sftpgo-current-probe-1.txt",
            "publisher-a/cover.jpg",
            "publisher-b/codex-sftpgo-current-probe-2.txt",
          ],
        },
        "rclone.deletefile": {
          describe: (args) => ({
            id: `approval-delete-${deletedTargets.length + 1}`,
            tool: "rclone.deletefile",
            summary: `Delete ${args.target}`,
            args,
          }),
          invoke: (args) => {
            deletedTargets.push(String(args.target));
            return { deleted: args.target };
          },
        },
      },
    });

    try {
      const loadResult = await session.eval(`
        ctx.files = tools.rclone.lsf({
          path: "spaces:globalcomix-publisher-uploads",
          recursive: true
        });
        ctx.files.length;
      `);
      const filterResult = await session.eval(`
        ctx.probes = ctx.files.containing("codex-sftpgo-current-probe");
        ctx.probes;
      `);
      const deleteResult = await session.eval(`
        ctx.deleted = [];
        for (const file of ctx.probes) {
          const target = "spaces:globalcomix-publisher-uploads/" + file;
          ctx.deleted.push(tools.rclone.deletefile({ target }).deleted);
        }
        ctx.deleted;
      `);

      expect(loadResult.value).toBe(3);
      expect(filterResult.value).toEqual([
        "publisher-a/codex-sftpgo-current-probe-1.txt",
        "publisher-b/codex-sftpgo-current-probe-2.txt",
      ]);
      expect(deleteResult.value).toEqual(deletedTargets);
      expect(deletedTargets).toEqual([
        "spaces:globalcomix-publisher-uploads/publisher-a/codex-sftpgo-current-probe-1.txt",
        "spaces:globalcomix-publisher-uploads/publisher-b/codex-sftpgo-current-probe-2.txt",
      ]);
      expect(approvals.map((approval) => approval.summary)).toEqual([
        "List spaces:globalcomix-publisher-uploads",
        "Delete spaces:globalcomix-publisher-uploads/publisher-a/codex-sftpgo-current-probe-1.txt",
        "Delete spaces:globalcomix-publisher-uploads/publisher-b/codex-sftpgo-current-probe-2.txt",
      ]);
    } finally {
      session.dispose();
    }
  });
});
