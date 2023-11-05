(function() {var type_impls = {
"x86_64":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#131-140\">source</a><a href=\"#impl-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, A&gt; <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.new\" class=\"method\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#134-139\">source</a><h4 class=\"code-header\">pub const fn <a href=\"x86_64/instructions/port/struct.PortGeneric.html#tymethod.new\" class=\"fn\">new</a>(port: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.u16.html\">u16</a>) -&gt; <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h4></section></summary><div class=\"docblock\"><p>Creates an I/O port with the given port number.</p>\n</div></details></div></details>",0,"x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#142-153\">source</a><a href=\"#impl-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"x86_64/structures/port/trait.PortRead.html\" title=\"trait x86_64::structures::port::PortRead\">PortRead</a>, A: <a class=\"trait\" href=\"x86_64/instructions/port/trait.PortReadAccess.html\" title=\"trait x86_64::instructions::port::PortReadAccess\">PortReadAccess</a>&gt; <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.read\" class=\"method\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#150-152\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"x86_64/instructions/port/struct.PortGeneric.html#tymethod.read\" class=\"fn\">read</a>(&amp;mut self) -&gt; T</h4></section></summary><div class=\"docblock\"><p>Reads from the port.</p>\n<h6 id=\"safety\"><a href=\"#safety\">Safety</a></h6>\n<p>This function is unsafe because the I/O port could have side effects that violate memory\nsafety.</p>\n</div></details></div></details>",0,"x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#155-166\">source</a><a href=\"#impl-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"x86_64/structures/port/trait.PortWrite.html\" title=\"trait x86_64::structures::port::PortWrite\">PortWrite</a>, A: <a class=\"trait\" href=\"x86_64/instructions/port/trait.PortWriteAccess.html\" title=\"trait x86_64::instructions::port::PortWriteAccess\">PortWriteAccess</a>&gt; <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.write\" class=\"method\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#163-165\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"x86_64/instructions/port/struct.PortGeneric.html#tymethod.write\" class=\"fn\">write</a>(&amp;mut self, value: T)</h4></section></summary><div class=\"docblock\"><p>Writes to the port.</p>\n<h6 id=\"safety\"><a href=\"#safety\">Safety</a></h6>\n<p>This function is unsafe because the I/O port could have side effects that violate memory\nsafety.</p>\n</div></details></div></details>",0,"x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Debug-for-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#168-176\">source</a><a href=\"#impl-Debug-for-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, A: Access&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html\" title=\"trait core::fmt::Debug\">Debug</a> for <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.fmt\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#169-175\">source</a><a href=\"#method.fmt\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html#tymethod.fmt\" class=\"fn\">fmt</a>(&amp;self, f: &amp;mut <a class=\"struct\" href=\"https://doc.rust-lang.org/nightly/core/fmt/struct.Formatter.html\" title=\"struct core::fmt::Formatter\">Formatter</a>&lt;'_&gt;) -&gt; <a class=\"type\" href=\"https://doc.rust-lang.org/nightly/core/fmt/type.Result.html\" title=\"type core::fmt::Result\">Result</a></h4></section></summary><div class='docblock'>Formats the value using the given formatter. <a href=\"https://doc.rust-lang.org/nightly/core/fmt/trait.Debug.html#tymethod.fmt\">Read more</a></div></details></div></details>","Debug","x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-PartialEq-for-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#187-191\">source</a><a href=\"#impl-PartialEq-for-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, A&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html\" title=\"trait core::cmp::PartialEq\">PartialEq</a> for <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.eq\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#188-190\">source</a><a href=\"#method.eq\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html#tymethod.eq\" class=\"fn\">eq</a>(&amp;self, other: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.reference.html\">&amp;Self</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.bool.html\">bool</a></h4></section></summary><div class='docblock'>This method tests for <code>self</code> and <code>other</code> values to be equal, and is used\nby <code>==</code>.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.ne\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/cmp.rs.html#239\">source</a></span><a href=\"#method.ne\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/cmp/trait.PartialEq.html#method.ne\" class=\"fn\">ne</a>(&amp;self, other: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.reference.html\">&amp;Rhs</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.bool.html\">bool</a></h4></section></summary><div class='docblock'>This method tests for <code>!=</code>. The default implementation is almost always\nsufficient, and should not be overridden without very good reason.</div></details></div></details>","PartialEq","x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<section id=\"impl-Eq-for-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#193\">source</a><a href=\"#impl-Eq-for-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, A&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/cmp/trait.Eq.html\" title=\"trait core::cmp::Eq\">Eq</a> for <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section>","Eq","x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Clone-for-PortGeneric%3CT,+A%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#178-185\">source</a><a href=\"#impl-Clone-for-PortGeneric%3CT,+A%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, A&gt; <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html\" title=\"trait core::clone::Clone\">Clone</a> for <a class=\"struct\" href=\"x86_64/instructions/port/struct.PortGeneric.html\" title=\"struct x86_64::instructions::port::PortGeneric\">PortGeneric</a>&lt;T, A&gt;</h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/x86_64/instructions/port.rs.html#179-184\">source</a><a href=\"#method.clone\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html#tymethod.clone\" class=\"fn\">clone</a>(&amp;self) -&gt; Self</h4></section></summary><div class='docblock'>Returns a copy of the value. <a href=\"https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html#tymethod.clone\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone_from\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/nightly/src/core/clone.rs.html#169\">source</a></span><a href=\"#method.clone_from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html#method.clone_from\" class=\"fn\">clone_from</a>(&amp;mut self, source: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.reference.html\">&amp;Self</a>)</h4></section></summary><div class='docblock'>Performs copy-assignment from <code>source</code>. <a href=\"https://doc.rust-lang.org/nightly/core/clone/trait.Clone.html#method.clone_from\">Read more</a></div></details></div></details>","Clone","x86_64::instructions::port::Port","x86_64::instructions::port::PortReadOnly","x86_64::instructions::port::PortWriteOnly"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()