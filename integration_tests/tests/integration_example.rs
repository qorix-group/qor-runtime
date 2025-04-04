// Copyright (c) 2025 Qorix GmbH
//
// This program and the accompanying materials are made available under the
// terms of the Apache License, Version 2.0 which is available at
// https://www.apache.org/licenses/LICENSE-2.0.
//
// SPDX-License-Identifier: Apache-2.0
//

#[cfg(test)]
mod tests {
    use orchestration::sub;

    #[test]
    fn integration_1() {
        assert_eq!(sub(3, 3), 0);
    }
}
