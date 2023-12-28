(function() {var type_impls = {
"hyperion_scheduler":[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Channel%3CT%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#137-236\">source</a><a href=\"#impl-Channel%3CT%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T&gt; <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html\" title=\"struct hyperion_scheduler::ipc::pipe::Channel\">Channel</a>&lt;T&gt;</h3></section></summary><div class=\"impl-items\"><section id=\"method.new\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#138-155\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.new\" class=\"fn\">new</a>(capacity: <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>) -&gt; Self</h4></section><section id=\"method.split\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#157-160\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.split\" class=\"fn\">split</a>(self) -&gt; (<a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Sender.html\" title=\"struct hyperion_scheduler::ipc::pipe::Sender\">Sender</a>&lt;T&gt;, <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Receiver.html\" title=\"struct hyperion_scheduler::ipc::pipe::Receiver\">Receiver</a>&lt;T&gt;)</h4></section><section id=\"method.send\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#162-181\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.send\" class=\"fn\">send</a>(&amp;self, item: T) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.unit.html\">()</a>, <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Closed.html\" title=\"struct hyperion_scheduler::ipc::pipe::Closed\">Closed</a>&gt;</h4></section><section id=\"method.recv\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#183-199\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.recv\" class=\"fn\">recv</a>(&amp;self) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;T, <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Closed.html\" title=\"struct hyperion_scheduler::ipc::pipe::Closed\">Closed</a>&gt;</h4></section><details class=\"toggle method-toggle\" open><summary><section id=\"method.send_closed\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#202-211\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.send_closed\" class=\"fn\">send_closed</a>(&amp;self)</h4></section></summary><div class=\"docblock\"><p>wait for the sender to be closed</p>\n</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.recv_closed\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#214-223\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.recv_closed\" class=\"fn\">recv_closed</a>(&amp;self)</h4></section></summary><div class=\"docblock\"><p>wait for the receiver to be closed</p>\n</div></details></div></details>",0,"hyperion_scheduler::ipc::pipe::Pipe"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Channel%3CT%3E\" class=\"impl\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#238-292\">source</a><a href=\"#impl-Channel%3CT%3E\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;T&gt; <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html\" title=\"struct hyperion_scheduler::ipc::pipe::Channel\">Channel</a>&lt;T&gt;<div class=\"where\">where\n    T: <a class=\"trait\" href=\"https://doc.rust-lang.org/nightly/core/marker/trait.Copy.html\" title=\"trait core::marker::Copy\">Copy</a>,</div></h3></section></summary><div class=\"impl-items\"><section id=\"method.send_slice\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#242-267\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.send_slice\" class=\"fn\">send_slice</a>(&amp;self, data: &amp;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.unit.html\">()</a>, <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Closed.html\" title=\"struct hyperion_scheduler::ipc::pipe::Closed\">Closed</a>&gt;</h4></section><section id=\"method.recv_slice\" class=\"method\"><a class=\"src rightside\" href=\"src/hyperion_scheduler/ipc/pipe.rs.html#269-291\">source</a><h4 class=\"code-header\">pub fn <a href=\"hyperion_scheduler/ipc/pipe/struct.Channel.html#tymethod.recv_slice\" class=\"fn\">recv_slice</a>(&amp;self, buf: &amp;mut <a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.slice.html\">[T]</a>) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/nightly/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;<a class=\"primitive\" href=\"https://doc.rust-lang.org/nightly/core/primitive.usize.html\">usize</a>, <a class=\"struct\" href=\"hyperion_scheduler/ipc/pipe/struct.Closed.html\" title=\"struct hyperion_scheduler::ipc::pipe::Closed\">Closed</a>&gt;</h4></section></div></details>",0,"hyperion_scheduler::ipc::pipe::Pipe"]]
};if (window.register_type_impls) {window.register_type_impls(type_impls);} else {window.pending_type_impls = type_impls;}})()