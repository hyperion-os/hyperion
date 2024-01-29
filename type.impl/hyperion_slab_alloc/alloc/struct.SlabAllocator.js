(function() {var type_impls = {
"hyperion_mem":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.new\" class=\"method\"><h4 class=\"code-header\">pub const fn <a class=\"fn\">new</a>() -&gt; SlabAllocator&lt;P, Lock&gt;</h4></section><section id=\"method.get_slab\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_slab</a>(&amp;self, size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>, &amp;Slab&lt;P, Lock&gt;)&gt;</h4></section></div></details>",0,"hyperion_mem::KernelSlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    P: AllocBackend,\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.alloc\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">alloc</a>(&amp;self, size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.free\" class=\"method\"><h4 class=\"code-header\">pub unsafe fn <a class=\"fn\">free</a>(&amp;self, alloc: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/ptr/non_null/struct.NonNull.html\" title=\"struct core::ptr::non_null::NonNull\">NonNull</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>&gt;)</h4></section></summary><div class=\"docblock\"><h5 id=\"safety\"><a class=\"doc-anchor\" href=\"#safety\">§</a>Safety</h5>\n<p><code>alloc</code> must point to an allocation that was previously allocated\nwith this specific [<code>SlabAllocator</code>]</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.size\" class=\"method\"><h4 class=\"code-header\">pub unsafe fn <a class=\"fn\">size</a>(&amp;self, alloc: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/ptr/non_null/struct.NonNull.html\" title=\"struct core::ptr::non_null::NonNull\">NonNull</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>&gt;) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><h5 id=\"safety-1\"><a class=\"doc-anchor\" href=\"#safety-1\">§</a>Safety</h5>\n<p><code>alloc</code> must point to an allocation that was previously allocated\nwith this specific [<code>SlabAllocator</code>]</p>\n</div></details></div></details>",0,"hyperion_mem::KernelSlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-GlobalAlloc-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-GlobalAlloc-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html\" title=\"trait core::alloc::global::GlobalAlloc\">GlobalAlloc</a> for SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    P: AllocBackend,\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.alloc\" class=\"method trait-impl\"><a href=\"#method.alloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.alloc\" class=\"fn\">alloc</a>(&amp;self, layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Allocate memory as described by the given <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.alloc\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.dealloc\" class=\"method trait-impl\"><a href=\"#method.dealloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.dealloc\" class=\"fn\">dealloc</a>(&amp;self, ptr: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>, _layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>)</h4></section></summary><div class='docblock'>Deallocate the block of memory at the given <code>ptr</code> pointer with the given <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.dealloc\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.alloc_zeroed\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.28.0\">1.28.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/alloc/global.rs.html#194\">source</a></span><a href=\"#method.alloc_zeroed\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.alloc_zeroed\" class=\"fn\">alloc_zeroed</a>(&amp;self, layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Behaves like <code>alloc</code>, but also ensures that the contents\nare set to zero before being returned. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.alloc_zeroed\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.realloc\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.28.0\">1.28.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/alloc/global.rs.html#263\">source</a></span><a href=\"#method.realloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.realloc\" class=\"fn\">realloc</a>(\n    &amp;self,\n    ptr: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>,\n    layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>,\n    new_size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>\n) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Shrink or grow a block of memory to the given <code>new_size</code> in bytes.\nThe block is described by the given <code>ptr</code> pointer and <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.realloc\">Read more</a></div></details></div></details>","GlobalAlloc","hyperion_mem::KernelSlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Default-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-Default-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html\" title=\"trait core::default::Default\">Default</a> for SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.default\" class=\"method trait-impl\"><a href=\"#method.default\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html#tymethod.default\" class=\"fn\">default</a>() -&gt; SlabAllocator&lt;P, Lock&gt;</h4></section></summary><div class='docblock'>Returns the “default value” for a type. <a href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html#tymethod.default\">Read more</a></div></details></div></details>","Default","hyperion_mem::KernelSlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-DerefMut-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-DerefMut-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.DerefMut.html\" title=\"trait core::ops::deref::DerefMut\">DerefMut</a> for SlabAllocator&lt;P, Lock&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.deref_mut\" class=\"method trait-impl\"><a href=\"#method.deref_mut\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.DerefMut.html#tymethod.deref_mut\" class=\"fn\">deref_mut</a>(&amp;mut self) -&gt; &amp;mut &lt;SlabAllocator&lt;P, Lock&gt; as <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a>&gt;::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" title=\"type core::ops::deref::Deref::Target\">Target</a></h4></section></summary><div class='docblock'>Mutably dereferences the value.</div></details></div></details>","DerefMut","hyperion_mem::KernelSlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Deref-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-Deref-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a> for SlabAllocator&lt;P, Lock&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle\" open><summary><section id=\"associatedtype.Target\" class=\"associatedtype trait-impl\"><a href=\"#associatedtype.Target\" class=\"anchor\">§</a><h4 class=\"code-header\">type <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" class=\"associatedtype\">Target</a> = SlabAllocatorStats</h4></section></summary><div class='docblock'>The resulting type after dereferencing.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.deref\" class=\"method trait-impl\"><a href=\"#method.deref\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#tymethod.deref\" class=\"fn\">deref</a>(&amp;self) -&gt; &amp;&lt;SlabAllocator&lt;P, Lock&gt; as <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a>&gt;::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" title=\"type core::ops::deref::Deref::Target\">Target</a></h4></section></summary><div class='docblock'>Dereferences the value.</div></details></div></details>","Deref","hyperion_mem::KernelSlabAlloc"]],
"libstd":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.new\" class=\"method\"><h4 class=\"code-header\">pub const fn <a class=\"fn\">new</a>() -&gt; SlabAllocator&lt;P, Lock&gt;</h4></section><section id=\"method.get_slab\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">get_slab</a>(&amp;self, size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;(<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>, &amp;Slab&lt;P, Lock&gt;)&gt;</h4></section></div></details>",0,"libstd::alloc::SlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    P: AllocBackend,\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.alloc\" class=\"method\"><h4 class=\"code-header\">pub fn <a class=\"fn\">alloc</a>(&amp;self, size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.free\" class=\"method\"><h4 class=\"code-header\">pub unsafe fn <a class=\"fn\">free</a>(&amp;self, alloc: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/ptr/non_null/struct.NonNull.html\" title=\"struct core::ptr::non_null::NonNull\">NonNull</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>&gt;)</h4></section></summary><div class=\"docblock\"><h5 id=\"safety\"><a class=\"doc-anchor\" href=\"#safety\">§</a>Safety</h5>\n<p><code>alloc</code> must point to an allocation that was previously allocated\nwith this specific [<code>SlabAllocator</code>]</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.size\" class=\"method\"><h4 class=\"code-header\">pub unsafe fn <a class=\"fn\">size</a>(&amp;self, alloc: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/ptr/non_null/struct.NonNull.html\" title=\"struct core::ptr::non_null::NonNull\">NonNull</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>&gt;) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><h5 id=\"safety-1\"><a class=\"doc-anchor\" href=\"#safety-1\">§</a>Safety</h5>\n<p><code>alloc</code> must point to an allocation that was previously allocated\nwith this specific [<code>SlabAllocator</code>]</p>\n</div></details></div></details>",0,"libstd::alloc::SlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-GlobalAlloc-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-GlobalAlloc-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html\" title=\"trait core::alloc::global::GlobalAlloc\">GlobalAlloc</a> for SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    P: AllocBackend,\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.alloc\" class=\"method trait-impl\"><a href=\"#method.alloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.alloc\" class=\"fn\">alloc</a>(&amp;self, layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Allocate memory as described by the given <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.alloc\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.dealloc\" class=\"method trait-impl\"><a href=\"#method.dealloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.dealloc\" class=\"fn\">dealloc</a>(&amp;self, ptr: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>, _layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>)</h4></section></summary><div class='docblock'>Deallocate the block of memory at the given <code>ptr</code> pointer with the given <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#tymethod.dealloc\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.alloc_zeroed\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.28.0\">1.28.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/alloc/global.rs.html#194\">source</a></span><a href=\"#method.alloc_zeroed\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.alloc_zeroed\" class=\"fn\">alloc_zeroed</a>(&amp;self, layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Behaves like <code>alloc</code>, but also ensures that the contents\nare set to zero before being returned. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.alloc_zeroed\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.realloc\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.28.0\">1.28.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/alloc/global.rs.html#263\">source</a></span><a href=\"#method.realloc\" class=\"anchor\">§</a><h4 class=\"code-header\">unsafe fn <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.realloc\" class=\"fn\">realloc</a>(\n    &amp;self,\n    ptr: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a>,\n    layout: <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/alloc/layout/struct.Layout.html\" title=\"struct core::alloc::layout::Layout\">Layout</a>,\n    new_size: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>\n) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.pointer.html\">*mut </a><a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u8.html\">u8</a></h4></section></summary><div class='docblock'>Shrink or grow a block of memory to the given <code>new_size</code> in bytes.\nThe block is described by the given <code>ptr</code> pointer and <code>layout</code>. <a href=\"https://doc.rust-lang.org/nightly/core/alloc/global/trait.GlobalAlloc.html#method.realloc\">Read more</a></div></details></div></details>","GlobalAlloc","libstd::alloc::SlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Default-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-Default-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html\" title=\"trait core::default::Default\">Default</a> for SlabAllocator&lt;P, Lock&gt;<div class=\"where\">where\n    Lock: RawMutex,</div></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.default\" class=\"method trait-impl\"><a href=\"#method.default\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html#tymethod.default\" class=\"fn\">default</a>() -&gt; SlabAllocator&lt;P, Lock&gt;</h4></section></summary><div class='docblock'>Returns the “default value” for a type. <a href=\"https://doc.rust-lang.org/nightly/core/default/trait.Default.html#tymethod.default\">Read more</a></div></details></div></details>","Default","libstd::alloc::SlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-DerefMut-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-DerefMut-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.DerefMut.html\" title=\"trait core::ops::deref::DerefMut\">DerefMut</a> for SlabAllocator&lt;P, Lock&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.deref_mut\" class=\"method trait-impl\"><a href=\"#method.deref_mut\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.DerefMut.html#tymethod.deref_mut\" class=\"fn\">deref_mut</a>(&amp;mut self) -&gt; &amp;mut &lt;SlabAllocator&lt;P, Lock&gt; as <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a>&gt;::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" title=\"type core::ops::deref::Deref::Target\">Target</a></h4></section></summary><div class='docblock'>Mutably dereferences the value.</div></details></div></details>","DerefMut","libstd::alloc::SlabAlloc"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Deref-for-SlabAllocator%3CP,+Lock%3E\" class=\"impl\"><a href=\"#impl-Deref-for-SlabAllocator%3CP,+Lock%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;P, Lock&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a> for SlabAllocator&lt;P, Lock&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle\" open><summary><section id=\"associatedtype.Target\" class=\"associatedtype trait-impl\"><a href=\"#associatedtype.Target\" class=\"anchor\">§</a><h4 class=\"code-header\">type <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" class=\"associatedtype\">Target</a> = SlabAllocatorStats</h4></section></summary><div class='docblock'>The resulting type after dereferencing.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.deref\" class=\"method trait-impl\"><a href=\"#method.deref\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#tymethod.deref\" class=\"fn\">deref</a>(&amp;self) -&gt; &amp;&lt;SlabAllocator&lt;P, Lock&gt; as <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html\" title=\"trait core::ops::deref::Deref\">Deref</a>&gt;::<a class=\"associatedtype\" href=\"https://doc.rust-lang.org/nightly/core/ops/deref/trait.Deref.html#associatedtype.Target\" title=\"type core::ops::deref::Deref::Target\">Target</a></h4></section></summary><div class='docblock'>Dereferences the value.</div></details></div></details>","Deref","libstd::alloc::SlabAlloc"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()