# Changes to the `types` Package

Aptos's base types definition package. Adds a DNS-name resolution method for network addresses.

---

## `types/src/network_address/mod.rs`

### Change 1: Add the `find_dns` method
- **New method**: `pub fn find_dns(&self) -> Option<String>`
- **Content**: Searches the protocol stack inside `NetworkAddress` for a DNS name (`Dns`, `Dns4`, `Dns6` protocols) and returns the DNS name string if found
- **Purpose**: Extract hostnames from validator network addresses
- **Background**: Used in oinori's `oinori_filter_sort_eligible_peers` function when determining validator geolocation. Validator network addresses may contain DNS names, for example `node2.cloud-b.mainnet.aptoslabs.com`, and the geolocation can be inferred from those DNS names
- **Similar methods**: Same pattern as the existing `find_ip_addr()` (IP address lookup) and `find_port()` (port lookup)

```diff
@@ -387,6 +387,16 @@ impl NetworkAddress {
         })
     }

+    /// oinori: find DNS name in the network address
+    pub fn find_dns(&self) -> Option<String> {
+        self.0.iter().find_map(|proto| match proto {
+            Protocol::Dns(dns_name) | Protocol::Dns4(dns_name) | Protocol::Dns6(dns_name) => {
+                Some(dns_name.0.clone())
+            },
+            _ => None,
+        })
+    }
+
     /// Retrieves the port from the network address
     pub fn find_port(&self) -> Option<u16> {
 ```

### Change 2: Formatting changes in test code
- **Change**: Reformatted array literals inside test cases to match rustfmt-compatible formatting
- **Purpose**: Adjust formatting to match the output of the code formatter. No logical change.

```diff
@@ -949,24 +959,30 @@ mod test {
                     Handshake(123),
                 ],
             ),
-            ("/ip6/::1/tcp/0", vec![
-                Ip6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)),
-                Tcp(0),
-            ]),
-            ("/ip6/dead:beef::c0de/tcp/8080", vec![
-                Ip6(Ipv6Addr::new(0xDEAD, 0xBEEF, 0, 0, 0, 0, 0, 0xC0DE)),
-                Tcp(8080),
-            ]),
-            ("/dns/example.com/tcp/80", vec![
-                Dns(DnsName("example.com".to_owned())),
-                Tcp(80),
-            ]),
-            (&noise_addr_str, vec![
-                Dns(DnsName("example.com".to_owned())),
-                Tcp(1234),
-                NoiseIK(pubkey),
-                Handshake(5),
-            ]),
+            (
+                "/ip6/::1/tcp/0",
+                vec![Ip6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), Tcp(0)],
+            ),
+            (
+                "/ip6/dead:beef::c0de/tcp/8080",
+                vec![
+                    Ip6(Ipv6Addr::new(0xDEAD, 0xBEEF, 0, 0, 0, 0, 0, 0xC0DE)),
+                    Tcp(8080),
+                ],
+            ),
+            (
+                "/dns/example.com/tcp/80",
+                vec![Dns(DnsName("example.com".to_owned())), Tcp(80)],
+            ),
+            (
+                &noise_addr_str,
+                vec![
+                    Dns(DnsName("example.com".to_owned())),
+                    Tcp(1234),
+                    NoiseIK(pubkey),
+                    Handshake(5),
+                ],
+            ),
         ];
 ```
