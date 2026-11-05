using Ryujinx.Graphics.Shader.Decoders;
using Ryujinx.Graphics.Shader.IntermediateRepresentation;
using Ryujinx.Graphics.Shader.StructuredIr;
using Ryujinx.Graphics.Shader.CodeGen;
using System.Collections.Generic;
using System;
using System.IO;
using System.Text;

namespace Ryujinx.Graphics.Shader.Translation
{
    // Dev probe (xlate port oracle): XLATE_DUMP=<dir> writes text dumps per phase, XLATE_TAG names the job.
    public static class Probe
    {
        [ThreadStatic] public static string Tag;
        static readonly string Dir = Environment.GetEnvironmentVariable("XLATE_DUMP");
        public static bool On => Dir != null && Tag != null;

        // ResourceManager call log (replayed by the Rust tests to rebuild the resource state): "<key>.res.txt"
        internal static void Res(string line)
        {
            if (!On) return;
            File.AppendAllText(Path.Combine(Dir, Tag + ".res.txt"), line + "\n");
        }

        internal static void Write(string phase, string text)
        {
            if (!On) return;
            if (phase != "decoded") Res("@" + phase);
            File.AppendAllText(Path.Combine(Dir, Tag + "." + phase + ".txt"), text);
        }

        internal static void Decoded(DecodedProgram program, int size)
        {
            if (!On) return;
            var sb = new StringBuilder();
            sb.Append("== attempt\n");
            foreach (DecodedFunction f in program)
            {
                sb.Append($"func {f.Address:x} type={(int)f.Type} blocks={f.Blocks.Length}\n");
                foreach (Block b in f.Blocks)
                {
                    sb.Append($" block {b.Address:x}-{b.EndAddress:x} succ=[");
                    foreach (Block s in b.Successors) sb.Append($"{s.Address:x},");
                    sb.Append("] push=[");
                    foreach (var p in b.PushOpCodes) sb.Append($"{p.Op.Address:x},");
                    sb.Append("]\n");
                    foreach (InstOp op in b.OpCodes) sb.Append($"  {op.Address:x} {op.Name} {op.RawOpCode:x16}\n");
                }
            }
            sb.Append($"features={(int)program.UsedFeatures} clip={program.ClipDistancesWritten} cb1={program.Cb1DataSize} size={size}\n");
            sb.Append($"attr in={program.AttributeUsage.UsedInputAttributes:x} out={program.AttributeUsage.UsedOutputAttributes:x}\n");
            Write("decoded", sb.ToString());
        }

        // ---- IR text (the contract with nxb-xlate ir/text.rs) ----
        // operand: _ null | a<n> argument | #<hex> constant | c<slot>:<off> cbuf | L<k> label | l<k> local | R<type>:<idx> register | undef
        // op line: "op <inst hex> <storagekind> <precise 0/1> <index> d=<a,b> s=<a,b>" and for texture ops " t=<type>,<format>,<flags>,<set>,<binding>,<sset>,<sbind>"
        // phi line: "phi <dest> <block>:<operand>,..." ; comment line: "cm <text>"
        sealed class Names
        {
            public Dictionary<Operand, int> Locals = new();
            public Dictionary<Operand, int> Labels = new();
            public string Of(Operand o)
            {
                if (o == null) return "_";
                switch (o.Type)
                {
                    case OperandType.Argument: return "a" + o.Value;
                    case OperandType.Constant: return "#" + ((uint)o.Value).ToString("x");
                    case OperandType.ConstantBuffer: return "c" + o.GetCbufSlot() + ":" + o.GetCbufOffset();
                    case OperandType.Label:
                        if (!Labels.TryGetValue(o, out int k)) { k = Labels.Count; Labels.Add(o, k); }
                        return "L" + k;
                    case OperandType.LocalVariable:
                        if (!Locals.TryGetValue(o, out int j)) { j = Locals.Count; Locals.Add(o, j); }
                        return "l" + j;
                    case OperandType.Register: { var r = o.GetRegister(); return "R" + (int)r.Type + ":" + r.Index; }
                    case OperandType.Undefined: return "undef";
                }
                return "?";
            }
        }

        static void Node(StringBuilder sb, INode n, Names nm, Func<BasicBlock, int> blockIndex)
        {
            if (n is CommentNode c) { sb.Append("  cm ").Append(c.Comment.Replace("\n", " ")).Append('\n'); return; }
            if (n is PhiNode p)
            {
                sb.Append("  phi ").Append(nm.Of(p.Dest)).Append(' ');
                for (int i = 0; i < p.SourcesCount; i++) sb.Append(blockIndex(p.GetBlock(i))).Append(':').Append(nm.Of(p.GetSource(i))).Append(',');
                sb.Append('\n');
                return;
            }
            var op = (Operation)n;
            sb.Append("  op ").Append(((uint)op.Inst).ToString("x")).Append(' ').Append((int)op.StorageKind).Append(' ').Append(op.ForcePrecise ? 1 : 0).Append(' ').Append(op.Index).Append(" d=");
            for (int i = 0; i < op.DestsCount; i++) sb.Append(nm.Of(op.GetDest(i))).Append(',');
            sb.Append(" s=");
            for (int i = 0; i < op.SourcesCount; i++) sb.Append(nm.Of(op.GetSource(i))).Append(',');
            if (op is TextureOperation t)
                sb.Append($" t={(int)t.Type},{(int)t.Format},{(int)t.Flags},{t.Set},{t.Binding},{t.SamplerSet},{t.SamplerBinding}");
            sb.Append('\n');
        }

        internal static void Ops(string phase, int func, Operation[] ops)
        {
            if (!On) return;
            var sb = new StringBuilder();
            sb.Append("== attempt\nfunc " + func + "\n");
            var nm = new Names();
            foreach (var o in ops) Node(sb, o, nm, null);
            Write(phase, sb.ToString());
        }

        internal static void Blocks(string phase, int func, BasicBlock[] blocks)
        {
            if (!On) return;
            var sb = new StringBuilder();
            sb.Append("== attempt\nfunc " + func + "\n");
            var nm = new Names();
            foreach (var b in blocks)
            {
                sb.Append(" block " + b.Index + " next=" + (b.Next?.Index ?? -1) + " branch=" + (b.Branch?.Index ?? -1) + " pred=");
                foreach (var pr in b.Predecessors) sb.Append(pr.Index).Append(',');
                sb.Append('\n');
                for (var n = b.Operations.First; n != null; n = n.Next) Node(sb, n.Value, nm, x => x.Index);
            }
            Write(phase, sb.ToString());
        }

        // ---- back end inputs (phases "props" and "ast"; the contract with nxb-xlate codegen/props.rs and structured/text.rs) ----
        // props: one attempt section per Generate call; lines "<keyword> <fields...>", see the Rust parser.
        // ast: the structured program after MakeStructuredProgram, as indented text.
        internal static void Props(StructuredProgramInfo info, CodeGenParameters p, ResourceManager rm)
        {
            if (!On) return;
            var d = p.Definitions;
            var sb = new StringBuilder("== attempt\n");
            int B(bool v) => v ? 1 : 0;
            sb.Append($"def stage={(int)d.Stage} local={d.ComputeLocalSizeX},{d.ComputeLocalSizeY},{d.ComputeLocalSizeZ} gp={B(d.GpPassthrough)} lastvtg={B(d.LastInVertexPipeline)} tpip={d.ThreadsPerInputPrimitive} intopo={(int)d.InputTopology} outtopo={(int)d.OutputTopology} maxout={d.MaxOutputVertices} dual={B(d.DualSourceBlend)} earlyz={B(d.EarlyZForce)} ynegate={B(d.YNegateEnabled)} originul={B(d.OriginUpperLeft)} tesscw={B(d.TessCw)} tessptype={(int)d.TessPatchType} tessspacing={(int)d.TessSpacing} iaidx={B(d.IaIndexing)} oaidx={B(d.OaIndexing)} tfb={B(d.TransformFeedbackEnabled)} scaled={B(d.SupportsScaledVertexFormats)}\n");
            if (d.ImapTypes != null)
                for (int i = 0; i < d.ImapTypes.Length; i++)
                    sb.Append($"imap {i} {(int)d.ImapTypes[i].X},{(int)d.ImapTypes[i].Y},{(int)d.ImapTypes[i].Z},{(int)d.ImapTypes[i].W}\n");
            for (int i = 0; i < 32; i++)
            {
                try { sb.Append($"attrtype {i} {(int)d.GetAttributeType(i)}\n"); } catch (Exception) { break; }
            }
            for (int i = 0; i < 8; i++)
            {
                try { sb.Append($"fout {i} {(int)d.GetFragmentOutputColorType(i)}\n"); } catch (Exception) { break; }
            }
            var h = p.HostCapabilities;
            sb.Append($"host reduced={B(h.ReducedPrecision)} interlock={B(h.SupportsFragmentShaderInterlock)} orderingintel={B(h.SupportsFragmentShaderOrderingIntel)} gppass={B(h.SupportsGeometryShaderPassthrough)} ballot={B(h.SupportsShaderBallot)} barrierdiv={B(h.SupportsShaderBarrierDivergence)} float64={B(h.SupportsShaderFloat64)} nonuniform={B(h.SupportsShaderNonUniformIndexing)} shadowlod={B(h.SupportsTextureShadowLod)} viewportmask={B(h.SupportsViewportMask)} metalatomics={B(h.MetalImageAtomics)}\n");
            sb.Append($"au passthrough={p.AttributeUsage.PassthroughAttributes}\n");
            sb.Append($"target api={(int)p.TargetApi}\n");
            var pr = p.Properties;
            sb.Append($"physptr {B(pr.UsesPhysicalPointers)}\n");
            foreach (var (name, dict) in new[] { ("cbuf", pr.ConstantBuffers), ("sbuf", pr.StorageBuffers) })
                foreach (var b in dict.Values)
                {
                    sb.Append($"{name} {(int)b.Layout} {b.Set} {b.Binding} {b.Type.Fields.Length} {b.Name}\n");
                    foreach (var f in b.Type.Fields) sb.Append($"field {(int)f.Type} {f.ArrayLength} {f.Name}\n");
                }
            foreach (var (name, dict) in new[] { ("tex", pr.Textures), ("img", pr.Images) })
                foreach (var t in dict.Values)
                    sb.Append($"{name} {t.Set} {t.Binding} {t.ArrayLength} {B(t.Separate)} {(int)t.Type} {(int)t.Format} {(int)t.Flags} {t.Name}\n");
            foreach (var (name, dict) in new[] { ("lmem", pr.LocalMemories), ("smem", pr.SharedMemories) })
                foreach (var kv in dict)
                    sb.Append($"{name} {kv.Key} {(int)kv.Value.Type} {kv.Value.ArrayLength} {kv.Value.Name}\n");
            var cb = (SetBindingPair[])typeof(ResourceManager).GetField("_cbSlotToBindingMap", System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance).GetValue(rm);
            for (int i = 0; i < cb.Length; i++) if (cb[i].Binding >= 0) sb.Append($"cbmap {i} {cb[i].SetIndex} {cb[i].Binding}\n");
            sb.Append($"helper {(int)info.HelperFunctionsMask}\n");
            foreach (var io in info.IoDefinitions) sb.Append($"io {(int)io.StorageKind} {(int)io.IoVariable} {io.Location} {io.Component}\n");
            Write("props", sb.ToString());
            Write("ast", AstText(info));
        }

        static string AstText(StructuredProgramInfo info)
        {
            var sb = new StringBuilder("== attempt\n");
            foreach (var f in info.Functions)
            {
                var ids = new Dictionary<AstOperand, int>();
                foreach (var l in f.Locals) ids[l] = ids.Count;
                sb.Append($"func {f.Name} ret={(int)f.ReturnType} in={string.Join(",", Array.ConvertAll(f.InArguments, x => ((int)x).ToString()))} out={string.Join(",", Array.ConvertAll(f.OutArguments, x => ((int)x).ToString()))}\n");
                foreach (var l in f.Locals) sb.Append($" local {ids[l]} {(int)l.VarType}\n");
                AstBlockText(sb, f.MainBlock, 1, ids);
            }
            return sb.ToString();
        }

        static void AstBlockText(StringBuilder sb, AstBlock b, int depth, Dictionary<AstOperand, int> ids)
        {
            string ind = new string(' ', depth);
            foreach (var n in b)
            {
                if (n is AstBlock c)
                {
                    sb.Append($"{ind}blk {c.Type} {(c.Condition == null ? "_" : AstExpr(c.Condition, ids))} {{\n");
                    AstBlockText(sb, c, depth + 1, ids);
                    sb.Append($"{ind}}}\n");
                }
                else if (n is AstAssignment a) sb.Append($"{ind}asg {AstExpr(a.Destination, ids)} = {AstExpr(a.Source, ids)}\n");
                else if (n is AstComment cm) sb.Append($"{ind}cm {cm.Comment.Replace("\n", " ")}\n");
                else sb.Append($"{ind}op {AstExpr(n, ids)}\n");
            }
        }

        static string AstExpr(IAstNode n, Dictionary<AstOperand, int> ids)
        {
            if (n == null) return "_";
            if (n is AstOperand o)
            {
                if (o.Type == OperandType.LocalVariable)
                {
                    if (!ids.TryGetValue(o, out int k)) { k = ids.Count; ids[o] = k; }
                    return $"l{k}:{(int)o.VarType}";
                }
                return o.Type switch { OperandType.Constant => "#" + ((uint)o.Value).ToString("x"), OperandType.Argument => "a" + o.Value, OperandType.Undefined => "undef", _ => "?" + o.Type };
            }
            var op = (AstOperation)n;
            var sb = new StringBuilder($"({((uint)op.Inst).ToString("x")} {(int)op.StorageKind} {(op.ForcePrecise ? 1 : 0)} {op.Index}");
            if (op is AstTextureOperation t) sb.Append($" t={(int)t.Type},{(int)t.Format},{(int)t.Flags},{t.Set},{t.Binding},{t.SamplerSet},{t.SamplerBinding}");
            for (int i = 0; i < op.SourcesCount; i++) sb.Append(' ').Append(AstExpr(op.GetSource(i), ids));
            return sb.Append(')').ToString();
        }

        // sfunc: every function with its blocks right before MakeStructuredProgram (helper functions included); sfuncmeta: "<i> <name> <returns> <in> <out>"
        internal static void SFuncs(IReadOnlyList<Function> funcs)
        {
            if (!On) return;
            var sb = new StringBuilder("== attempt\n");
            var meta = new StringBuilder("== attempt\n");
            for (int i = 0; i < funcs.Count; i++)
            {
                var f = funcs[i];
                meta.Append($"{i} {f.Name} {(f.ReturnsValue ? 1 : 0)} {f.InArgumentsCount} {f.OutArgumentsCount}\n");
                sb.Append("func " + i + "\n");
                var nm = new Names();
                foreach (var b in f.Blocks)
                {
                    sb.Append(" block " + b.Index + " next=" + (b.Next?.Index ?? -1) + " branch=" + (b.Branch?.Index ?? -1) + " pred=");
                    foreach (var pr in b.Predecessors) sb.Append(pr.Index).Append(',');
                    sb.Append('\n');
                    for (var n = b.Operations.First; n != null; n = n.Next) Node(sb, n.Value, nm, x => x.Index);
                }
            }
            // suses: for every local (numbered as in the blocks text) the nodes of UseOps in enumeration order; node ids count the nodes of all functions in print order, - for a node no block holds
            var uses = new StringBuilder("== attempt\n");
            var ids = new Dictionary<INode, int>();
            foreach (var f in funcs) foreach (var b in f.Blocks) for (var n = b.Operations.First; n != null; n = n.Next) ids[n.Value] = ids.Count;
            for (int i = 0; i < funcs.Count; i++)
            {
                var order = new List<Operand>();
                var seen = new HashSet<Operand>();
                void See(Operand o) { if (o != null && o.Type == OperandType.LocalVariable && seen.Add(o)) order.Add(o); }
                foreach (var b in funcs[i].Blocks)
                    for (var n = b.Operations.First; n != null; n = n.Next)
                    {
                        if (n.Value is PhiNode p) { See(p.Dest); for (int k = 0; k < p.SourcesCount; k++) See(p.GetSource(k)); }
                        else if (n.Value is Operation op) { for (int k = 0; k < op.DestsCount; k++) See(op.GetDest(k)); for (int k = 0; k < op.SourcesCount; k++) See(op.GetSource(k)); }
                    }
                uses.Append("f " + i + "\n");
                for (int k = 0; k < order.Count; k++)
                {
                    uses.Append("u " + k + " ").Append(UseSlots(order[k].UseOps, ids)).Append('\n');
                }
            }
            Write("suses", uses.ToString());
            Write("sfunc", sb.ToString());
            Write("sfuncmeta", meta.ToString());
        }

        // the raw slots of a HashSet (holes as _, the free list chain after "free=") so the order of later additions is reproducible
        static string UseSlots(HashSet<INode> set, Dictionary<INode, int> ids)
        {
            const System.Reflection.BindingFlags F = System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance;
            var t = typeof(HashSet<INode>);
            var entries = (Array)t.GetField("_entries", F).GetValue(set);
            if (entries == null) return "free=";
            int count = (int)t.GetField("_count", F).GetValue(set);
            int freeList = (int)t.GetField("_freeList", F).GetValue(set);
            var et = entries.GetType().GetElementType();
            var fNext = et.GetField("Next", System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            var fVal = et.GetField("Value", System.Reflection.BindingFlags.Public | System.Reflection.BindingFlags.NonPublic | System.Reflection.BindingFlags.Instance);
            var sb = new StringBuilder();
            for (int i = 0; i < count; i++)
            {
                var e = entries.GetValue(i);
                if ((int)fNext.GetValue(e) < -1) sb.Append("_,");
                else sb.Append(ids.TryGetValue((INode)fVal.GetValue(e), out int id) ? id.ToString() : "-").Append(',');
            }
            sb.Append(" free=");
            for (int f = freeList; f >= 0; f = -3 - (int)fNext.GetValue(entries.GetValue(f))) sb.Append(f).Append(',');
            return sb.ToString();
        }
    }
}
