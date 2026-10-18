//! Builds the starter cart and exposes the SDK to other projects:
//!
//!     zig build          zig-out/bin/starter.wasm
//!     zig build test     unit tests for the SDK's pure helpers
//!
//! In your own build.zig, after `zig fetch --save` of this package:
//!
//!     const wasm_ad = @import("wasm_ad");
//!     const dep = b.dependency("wasm_ad", .{});
//!     const cart = wasm_ad.addCart(b, .{
//!         .name = "mygame",
//!         .root_source_file = b.path("src/main.zig"),
//!         .wasm_ad = dep.module("wasm_ad"),
//!     });
//!     b.installArtifact(cart);

const std = @import("std");

pub fn build(b: *std.Build) void {
    const optimize = b.option(std.builtin.OptimizeMode, "optimize", "Optimization mode (default ReleaseSmall)") orelse .ReleaseSmall;
    const wasm_ad = b.addModule("wasm_ad", .{ .root_source_file = b.path("src/wasm_ad.zig") });

    b.installArtifact(addCart(b, .{
        .name = "starter",
        .root_source_file = b.path("examples/starter.zig"),
        .wasm_ad = wasm_ad,
        .optimize = optimize,
    }));

    const tests = b.addTest(.{ .root_module = b.createModule(.{
        .root_source_file = b.path("src/wasm_ad.zig"),
        .target = b.graph.host,
    }) });
    b.step("test", "Run the SDK's unit tests").dependOn(&b.addRunArtifact(tests).step);
}

pub const CartOptions = struct {
    name: []const u8,
    root_source_file: std.Build.LazyPath,
    /// The SDK module: `b.dependency("wasm_ad", .{}).module("wasm_ad")`.
    wasm_ad: *std.Build.Module,
    optimize: std.builtin.OptimizeMode = .ReleaseSmall,
    /// Initial linear memory; at least 3 pages (SPEC.md §2.1).
    initial_memory: u64 = 4 * 65536,
    stack_size: u64 = 64 * 1024,
};

/// A cart executable with the layout SPEC.md requires: data from 0x28000 with
/// the stack above it (§4, §13), no entry point, and the `export fn`s
/// (`update`, `start`, `scanline`) exported.
pub fn addCart(b: *std.Build, options: CartOptions) *std.Build.Step.Compile {
    const target = b.resolveTargetQuery(.{ .cpu_arch = .wasm32, .os_tag = .freestanding });
    const root = b.createModule(.{
        .root_source_file = options.root_source_file,
        .target = target,
        .optimize = options.optimize,
        .imports = &.{.{ .name = "wasm_ad", .module = options.wasm_ad }},
    });
    const exe = b.addExecutable(.{ .name = options.name, .root_module = root });
    exe.entry = .disabled;
    exe.rdynamic = true;
    exe.global_base = 0x28000;
    exe.stack_size = options.stack_size;
    exe.initial_memory = options.initial_memory;
    return exe;
}
