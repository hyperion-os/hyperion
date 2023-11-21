(function() {var type_impls = {
"ringbuf":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Consumer%3CT,+R%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#28-242\">source</a><a href=\"#impl-Consumer%3CT,+R%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T, R: <a class=\"trait\" href=\"ringbuf/ring_buffer/trait.RbRef.html\" title=\"trait ringbuf::ring_buffer::RbRef\">RbRef</a>&gt; <a class=\"struct\" href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\">Consumer</a>&lt;T, R&gt;<span class=\"where fmt-newline\">where\n    R::<a class=\"associatedtype\" href=\"ringbuf/ring_buffer/trait.RbRef.html#associatedtype.Rb\" title=\"type ringbuf::ring_buffer::RbRef::Rb\">Rb</a>: <a class=\"trait\" href=\"ringbuf/ring_buffer/trait.RbRead.html\" title=\"trait ringbuf::ring_buffer::RbRead\">RbRead</a>&lt;T&gt;,</span></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.new\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#37-42\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.new\" class=\"fn\">new</a>(target: R) -&gt; Self</h4></section></summary><div class=\"docblock\"><p>Creates consumer from the ring buffer reference.</p>\n<h5 id=\"safety\"><a href=\"#safety\">Safety</a></h5>\n<p>There must be only one consumer containing the same ring buffer reference.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.rb\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#46-48\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.rb\" class=\"fn\">rb</a>(&amp;self) -&gt; &amp;R::<a class=\"associatedtype\" href=\"ringbuf/ring_buffer/trait.RbRef.html#associatedtype.Rb\" title=\"type ringbuf::ring_buffer::RbRef::Rb\">Rb</a></h4></section></summary><div class=\"docblock\"><p>Returns reference to the underlying ring buffer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.into_rb_ref\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#51-53\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.into_rb_ref\" class=\"fn\">into_rb_ref</a>(self) -&gt; R</h4></section></summary><div class=\"docblock\"><p>Consumes <code>self</code> and returns underlying ring buffer reference.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.postponed\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#56-58\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.postponed\" class=\"fn\">postponed</a>(&amp;mut self) -&gt; <a class=\"struct\" href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\">Consumer</a>&lt;T, <a class=\"struct\" href=\"ringbuf/ring_buffer/struct.RbWrap.html\" title=\"struct ringbuf::ring_buffer::RbWrap\">RbWrap</a>&lt;<a class=\"struct\" href=\"ringbuf/ring_buffer/struct.RbReadCache.html\" title=\"struct ringbuf::ring_buffer::RbReadCache\">RbReadCache</a>&lt;T, &amp;R::<a class=\"associatedtype\" href=\"ringbuf/ring_buffer/trait.RbRef.html#associatedtype.Rb\" title=\"type ringbuf::ring_buffer::RbRef::Rb\">Rb</a>&gt;&gt;&gt;</h4></section></summary><div class=\"docblock\"><p>Returns postponed consumer that borrows <a href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\"><code>Self</code></a>.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.into_postponed\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#61-63\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.into_postponed\" class=\"fn\">into_postponed</a>(self) -&gt; <a class=\"struct\" href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\">Consumer</a>&lt;T, <a class=\"struct\" href=\"ringbuf/ring_buffer/struct.RbWrap.html\" title=\"struct ringbuf::ring_buffer::RbWrap\">RbWrap</a>&lt;<a class=\"struct\" href=\"ringbuf/ring_buffer/struct.RbReadCache.html\" title=\"struct ringbuf::ring_buffer::RbReadCache\">RbReadCache</a>&lt;T, R&gt;&gt;&gt;</h4></section></summary><div class=\"docblock\"><p>Transforms <a href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\"><code>Self</code></a> into postponed consumer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.capacity\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#69-71\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.capacity\" class=\"fn\">capacity</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>Returns capacity of the ring buffer.</p>\n<p>The capacity of the buffer is constant.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.is_empty\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#77-79\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.is_empty\" class=\"fn\">is_empty</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.bool.html\">bool</a></h4></section></summary><div class=\"docblock\"><p>Checks if the ring buffer is empty.</p>\n<p><em>The result may become irrelevant at any time because of concurring producer activity.</em></p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.is_full\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#83-85\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.is_full\" class=\"fn\">is_full</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.bool.html\">bool</a></h4></section></summary><div class=\"docblock\"><p>Checks if the ring buffer is full.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.len\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#91-93\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.len\" class=\"fn\">len</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>The number of items stored in the buffer.</p>\n<p><em>Actual number may be greater than the returned value because of concurring producer activity.</em></p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.free_len\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#99-101\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.free_len\" class=\"fn\">free_len</a>(&amp;self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>The number of remaining free places in the buffer.</p>\n<p><em>Actual number may be less than the returned value because of concurring producer activity.</em></p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.as_uninit_slices\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#117-120\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.as_uninit_slices\" class=\"fn\">as_uninit_slices</a>(&amp;self) -&gt; (&amp;[<a class=\"union\" href=\"https://doc.rust-lang.org/nightly/core/mem/maybe_uninit/union.MaybeUninit.html\" title=\"union core::mem::maybe_uninit::MaybeUninit\">MaybeUninit</a>&lt;T&gt;], &amp;[<a class=\"union\" href=\"https://doc.rust-lang.org/nightly/core/mem/maybe_uninit/union.MaybeUninit.html\" title=\"union core::mem::maybe_uninit::MaybeUninit\">MaybeUninit</a>&lt;T&gt;])</h4></section></summary><div class=\"docblock\"><p>Provides a direct access to the ring buffer occupied memory.\nThe difference from <a href=\"ringbuf/consumer/struct.Consumer.html#method.as_slices\" title=\"method ringbuf::consumer::Consumer::as_slices\"><code>Self::as_slices</code></a> is that this method provides slices of <a href=\"https://doc.rust-lang.org/nightly/core/mem/maybe_uninit/union.MaybeUninit.html\" title=\"union core::mem::maybe_uninit::MaybeUninit\"><code>MaybeUninit&lt;T&gt;</code></a>, so items may be moved out of slices.</p>\n<p>Returns a pair of slices of stored items, the second one may be empty.\nElements with lower indices in slice are older. First slice contains older items that second one.</p>\n<h5 id=\"safety-1\"><a href=\"#safety-1\">Safety</a></h5>\n<p>All items are initialized. Elements must be removed starting from the beginning of first slice.\nWhen all items are removed from the first slice then items must be removed from the beginning of the second slice.</p>\n<p><em>This method must be followed by <a href=\"ringbuf/consumer/struct.Consumer.html#method.advance\" title=\"method ringbuf::consumer::Consumer::advance\"><code>Self::advance</code></a> call with the number of items being removed previously as argument.</em>\n<em>No other mutating calls allowed before that.</em></p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.as_mut_uninit_slices\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#130-132\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.as_mut_uninit_slices\" class=\"fn\">as_mut_uninit_slices</a>(\n    &amp;self\n) -&gt; (&amp;mut [<a class=\"union\" href=\"https://doc.rust-lang.org/nightly/core/mem/maybe_uninit/union.MaybeUninit.html\" title=\"union core::mem::maybe_uninit::MaybeUninit\">MaybeUninit</a>&lt;T&gt;], &amp;mut [<a class=\"union\" href=\"https://doc.rust-lang.org/nightly/core/mem/maybe_uninit/union.MaybeUninit.html\" title=\"union core::mem::maybe_uninit::MaybeUninit\">MaybeUninit</a>&lt;T&gt;])</h4></section></summary><div class=\"docblock\"><p>Provides a direct mutable access to the ring buffer occupied memory.</p>\n<p>Same as <a href=\"ringbuf/consumer/struct.Consumer.html#method.as_uninit_slices\" title=\"method ringbuf::consumer::Consumer::as_uninit_slices\"><code>Self::as_uninit_slices</code></a>.</p>\n<h5 id=\"safety-2\"><a href=\"#safety-2\">Safety</a></h5>\n<p>See <a href=\"ringbuf/consumer/struct.Consumer.html#method.as_uninit_slices\" title=\"method ringbuf::consumer::Consumer::as_uninit_slices\"><code>Self::as_uninit_slices</code></a>.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.advance\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#140-142\">source</a><h4 class=\"code-header\">pub unsafe fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.advance\" class=\"fn\">advance</a>(&amp;mut self, count: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>)</h4></section></summary><div class=\"docblock\"><p>Moves <code>head</code> target by <code>count</code> places.</p>\n<h5 id=\"safety-3\"><a href=\"#safety-3\">Safety</a></h5>\n<p>First <code>count</code> items in occupied memory must be moved out or dropped.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.as_slices\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#146-151\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.as_slices\" class=\"fn\">as_slices</a>(&amp;self) -&gt; (&amp;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>, &amp;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>)</h4></section></summary><div class=\"docblock\"><p>Returns a pair of slices which contain, in order, the contents of the ring buffer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.as_mut_slices\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#155-160\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.as_mut_slices\" class=\"fn\">as_mut_slices</a>(&amp;mut self) -&gt; (&amp;mut <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>, &amp;mut <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>)</h4></section></summary><div class=\"docblock\"><p>Returns a pair of mutable slices which contain, in order, the contents of the ring buffer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.pop\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#165-178\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.pop\" class=\"fn\">pop</a>(&amp;mut self) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/option/enum.Option.html\" title=\"enum core::option::Option\">Option</a>&lt;T&gt;</h4></section></summary><div class=\"docblock\"><p>Removes latest item from the ring buffer and returns it.</p>\n<p>Returns <code>None</code> if the ring buffer is empty.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.pop_iter\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#185-187\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.pop_iter\" class=\"fn\">pop_iter</a>(&amp;mut self) -&gt; <a class=\"struct\" href=\"ringbuf/consumer/struct.PopIterator.html\" title=\"struct ringbuf::consumer::PopIterator\">PopIterator</a>&lt;'_, T, R&gt; <a href=\"#\" class=\"tooltip\" data-notable-ty=\"PopIterator&lt;&#39;_, T, R&gt;\">ⓘ</a></h4></section></summary><div class=\"docblock\"><p>Returns an iterator that removes items one by one from the ring buffer.</p>\n<p>Iterator provides only items that are available for consumer at the moment of <code>pop_iter</code> call, it will not contain new items added after it was created.</p>\n<p><em>Information about removed items is commited to the buffer only when iterator is destroyed.</em></p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.iter\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#192-195\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.iter\" class=\"fn\">iter</a>(&amp;self) -&gt; impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/iter/traits/iterator/trait.Iterator.html\" title=\"trait core::iter::traits::iterator::Iterator\">Iterator</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.reference.html\">&amp;T</a>&gt; + '_</h4></section></summary><div class=\"docblock\"><p>Returns a front-to-back iterator containing references to items in the ring buffer.</p>\n<p>This iterator does not remove items out of the ring buffer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.iter_mut\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#200-203\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.iter_mut\" class=\"fn\">iter_mut</a>(&amp;mut self) -&gt; impl <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/iter/traits/iterator/trait.Iterator.html\" title=\"trait core::iter::traits::iterator::Iterator\">Iterator</a>&lt;Item = <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.reference.html\">&amp;mut T</a>&gt; + '_</h4></section></summary><div class=\"docblock\"><p>Returns a front-to-back iterator that returns mutable references to items in the ring buffer.</p>\n<p>This iterator does not remove items out of the ring buffer.</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.skip\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#230-234\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.skip\" class=\"fn\">skip</a>(&amp;mut self, count: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>Removes at most <code>n</code> and at least <code>min(n, Self::len())</code> items from the buffer and safely drops them.</p>\n<p>If there is no concurring producer activity then exactly <code>min(n, Self::len())</code> items are removed.</p>\n<p>Returns the number of deleted items.</p>\n\n<div class=\"example-wrap\"><pre class=\"rust rust-example-rendered\"><code><span class=\"kw\">let </span>target = HeapRb::&lt;i32&gt;::new(<span class=\"number\">8</span>);\n<span class=\"kw\">let </span>(<span class=\"kw-2\">mut </span>prod, <span class=\"kw-2\">mut </span>cons) = target.split();\n\n<span class=\"macro\">assert_eq!</span>(prod.push_iter(<span class=\"kw-2\">&amp;mut </span>(<span class=\"number\">0</span>..<span class=\"number\">8</span>)), <span class=\"number\">8</span>);\n\n<span class=\"macro\">assert_eq!</span>(cons.skip(<span class=\"number\">4</span>), <span class=\"number\">4</span>);\n<span class=\"macro\">assert_eq!</span>(cons.skip(<span class=\"number\">8</span>), <span class=\"number\">4</span>);\n<span class=\"macro\">assert_eq!</span>(cons.skip(<span class=\"number\">8</span>), <span class=\"number\">0</span>);</code></pre></div>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.clear\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#239-241\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.clear\" class=\"fn\">clear</a>(&amp;mut self) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>Removes all items from the buffer and safely drops them.</p>\n<p>Returns the number of deleted items.</p>\n</div></details></div></details>",0,"ringbuf::alias::StaticConsumer","ringbuf::alias::HeapConsumer","ringbuf::consumer::PostponedConsumer"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Consumer%3CT,+R%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#312-340\">source</a><a href=\"#impl-Consumer%3CT,+R%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html\" title=\"trait core::marker::Copy\">Copy</a>, R: <a class=\"trait\" href=\"ringbuf/ring_buffer/trait.RbRef.html\" title=\"trait ringbuf::ring_buffer::RbRef\">RbRef</a>&gt; <a class=\"struct\" href=\"ringbuf/consumer/struct.Consumer.html\" title=\"struct ringbuf::consumer::Consumer\">Consumer</a>&lt;T, R&gt;<span class=\"where fmt-newline\">where\n    R::<a class=\"associatedtype\" href=\"ringbuf/ring_buffer/trait.RbRef.html#associatedtype.Rb\" title=\"type ringbuf::ring_buffer::RbRef::Rb\">Rb</a>: <a class=\"trait\" href=\"ringbuf/ring_buffer/trait.RbRead.html\" title=\"trait ringbuf::ring_buffer::RbRead\">RbRead</a>&lt;T&gt;,</span></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.pop_slice\" class=\"method\"><a class=\"src rightside\" href=\"src/ringbuf/consumer.rs.html#320-339\">source</a><h4 class=\"code-header\">pub fn <a href=\"ringbuf/consumer/struct.Consumer.html#tymethod.pop_slice\" class=\"fn\">pop_slice</a>(&amp;mut self, elems: &amp;mut <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a></h4></section></summary><div class=\"docblock\"><p>Removes first items from the ring buffer and writes them into a slice.\nElements must be <a href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html\" title=\"trait core::marker::Copy\"><code>Copy</code></a>.</p>\n<p>Returns count of items been removed from the ring buffer.</p>\n</div></details></div></details>",0,"ringbuf::alias::StaticConsumer","ringbuf::alias::HeapConsumer","ringbuf::consumer::PostponedConsumer"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()