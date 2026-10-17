const NOVATION_HEADER = [0x00, 0x20, 0x29, 0x00];

export const COMMAND = Object.freeze({
  INIT: 0x71,
  WRITE: 0x72,
  FINISH: 0x73,
  FOOTER: 0x76,
  HEADER: 0x7c,
});

function arraysEqual(actual, expected) {
  return (
    actual.length === expected.length &&
    expected.every((value, index) => actual[index] === value)
  );
}

function parseNibbles(data) {
  let result = 0;
  for (const value of data) {
    if (value > 0x0f) {
      throw new Error("Firmware metadata contains a non-nibble value");
    }
    result = result * 16 + value;
  }
  return result;
}

function unpackMidi7Bit(data) {
  let accumulator = 0;
  let bitCount = 0;
  const output = [];

  for (const value of data) {
    if (value > 0x7f) {
      throw new Error("Firmware block contains a non-MIDI data byte");
    }

    accumulator = (accumulator << 7) | value;
    bitCount += 7;

    while (bitCount >= 8) {
      bitCount -= 8;
      output.push((accumulator >>> bitCount) & 0xff);
      accumulator &= bitCount === 0 ? 0 : (1 << bitCount) - 1;
    }
  }

  if (bitCount !== 3 || accumulator !== 0) {
    throw new Error("Firmware block has invalid 7-bit padding");
  }

  return Uint8Array.from(output);
}

export function crc32Mpeg2(data) {
  let crc = 0xffffffff;
  for (const value of data) {
    crc = (crc ^ (value << 24)) >>> 0;
    for (let bit = 0; bit < 8; bit += 1) {
      crc =
        crc & 0x80000000
          ? ((crc << 1) ^ 0x04c11db7) >>> 0
          : (crc << 1) >>> 0;
    }
  }
  return crc >>> 0;
}

function splitSysEx(data) {
  const messages = [];
  let offset = 0;

  while (offset < data.length) {
    while (
      offset < data.length &&
      (data[offset] === 0x00 || data[offset] === 0x0a || data[offset] === 0x0d)
    ) {
      offset += 1;
    }
    if (offset === data.length) {
      break;
    }
    if (data[offset] !== 0xf0) {
      throw new Error(`Expected SysEx start at file offset 0x${offset.toString(16)}`);
    }

    let end = offset + 1;
    while (end < data.length && data[end] !== 0xf7) {
      end += 1;
    }
    if (end === data.length) {
      throw new Error(`Unterminated SysEx message at file offset 0x${offset.toString(16)}`);
    }

    const message = data.slice(offset, end + 1);
    if (message.length < 7 || !arraysEqual(message.slice(1, 5), NOVATION_HEADER)) {
      throw new Error(`Unexpected SysEx message at file offset 0x${offset.toString(16)}`);
    }
    messages.push(message);
    offset = end + 1;
  }

  return messages;
}

export function parseFirmwareSysEx(data) {
  const messages = splitSysEx(data);
  const byCommand = new Map();
  for (const message of messages) {
    const command = message[5];
    if (!Object.values(COMMAND).includes(command)) {
      throw new Error(`Unsupported update command 0x${command.toString(16)}`);
    }
    const commandMessages = byCommand.get(command) ?? [];
    commandMessages.push(message);
    byCommand.set(command, commandMessages);
  }

  const initMessages = byCommand.get(COMMAND.INIT) ?? [];
  const headerMessages = byCommand.get(COMMAND.HEADER) ?? [];
  const writeMessages = byCommand.get(COMMAND.WRITE) ?? [];
  const finishMessages = byCommand.get(COMMAND.FINISH) ?? [];
  const footerMessages = byCommand.get(COMMAND.FOOTER) ?? [];

  if (initMessages.length !== 1 || headerMessages.length !== 1 || finishMessages.length !== 1) {
    throw new Error("Firmware must contain exactly one INIT, HEADER, and FINISH message");
  }
  if (writeMessages.length === 0) {
    throw new Error("Firmware contains no WRITE messages");
  }
  if (footerMessages.length !== 0) {
    throw new Error("UPDATE_FOOTER is not supported by this sender");
  }

  const expectedOrder = [
    initMessages[0],
    headerMessages[0],
    ...writeMessages,
    finishMessages[0],
  ];
  if (
    messages.length !== expectedOrder.length ||
    messages.some((message, index) => message !== expectedOrder[index])
  ) {
    throw new Error("Firmware update messages are not in the expected order");
  }

  const initPayload = initMessages[0].slice(6, -1);
  const headerPayload = headerMessages[0].slice(6, -1);
  if (initPayload.length !== 8 || headerPayload.length !== 23) {
    throw new Error("Firmware identity metadata has an unexpected length");
  }

  const deviceId = initPayload[1];
  if (deviceId !== 0x63) {
    throw new Error(`Expected Circuit Rhythm device ID 0x63, got 0x${deviceId.toString(16)}`);
  }

  const initVersion = parseNibbles(initPayload.slice(2, 8));
  const version = parseNibbles(headerPayload.slice(1, 7));
  if (initVersion !== version) {
    throw new Error("INIT and HEADER firmware versions do not match");
  }

  const declaredSize = parseNibbles(headerPayload.slice(7, 15));
  const declaredCrc = parseNibbles(headerPayload.slice(15, 23)) >>> 0;
  const finishBlock = unpackMidi7Bit(finishMessages[0].slice(6, -1));
  const writeBlocks = writeMessages.map((message) =>
    unpackMidi7Bit(message.slice(6, -1)),
  );
  if (
    finishBlock.length !== 32 ||
    writeBlocks.some((block) => block.length !== 32)
  ) {
    throw new Error("Firmware messages do not contain 32-byte decoded blocks");
  }
  const decodedLength =
    finishBlock.length + writeBlocks.reduce((sum, block) => sum + block.length, 0);
  if (declaredSize > decodedLength || declaredSize <= decodedLength - 32) {
    throw new Error(
      `Firmware size ${declaredSize} is inconsistent with ${decodedLength} encoded bytes`,
    );
  }

  const firmware = new Uint8Array(declaredSize);
  let firmwareOffset = 0;
  for (const block of [finishBlock, ...writeBlocks]) {
    const remaining = declaredSize - firmwareOffset;
    if (remaining <= 0) {
      break;
    }
    const chunk = block.slice(0, remaining);
    firmware.set(chunk, firmwareOffset);
    firmwareOffset += chunk.length;
  }

  const calculatedCrc = crc32Mpeg2(firmware);
  if (calculatedCrc !== declaredCrc) {
    throw new Error(
      `Firmware CRC mismatch: calculated ${calculatedCrc
        .toString(16)
        .padStart(8, "0")}, expected ${declaredCrc.toString(16).padStart(8, "0")}`,
    );
  }

  return Object.freeze({
    initMessage: initMessages[0],
    headerMessage: headerMessages[0],
    writeMessages: Object.freeze(writeMessages),
    finishMessage: finishMessages[0],
    firmware,
    version,
    declaredSize,
    declaredCrc,
  });
}

function parseBootloaderMessage(data) {
  if (
    data.length < 7 ||
    data[0] !== 0xf0 ||
    data[data.length - 1] !== 0xf7 ||
    !arraysEqual(data.slice(1, 5), NOVATION_HEADER)
  ) {
    return null;
  }
  return {
    command: data[5],
    payload: data.slice(6, -1),
  };
}

export class RequestDrivenSender {
  constructor(
    firmware,
    send,
    {
      timeoutMs = 10000,
      onProgress = () => {},
      onStateChange = () => {},
    } = {},
  ) {
    this.firmware = firmware;
    this.send = send;
    this.timeoutMs = timeoutMs;
    this.onProgress = onProgress;
    this.onStateChange = onStateChange;
    this.state = "idle";
    this.nextBlock = 1;
    this.lastBlock = null;
    this.timer = null;
  }

  start() {
    if (this.state !== "idle") {
      throw new Error(`Cannot start sender in state ${this.state}`);
    }
    this.state = "waiting-for-request";
    this.onStateChange(this.state);
    this.send(this.firmware.initMessage);
    this.send(this.firmware.headerMessage);
    this.armTimeout("Timed out waiting for the bootloader's first block request");
  }

  handleMidi(data) {
    if (!["waiting-for-request", "waiting-for-finish"].includes(this.state)) {
      return false;
    }

    const message = parseBootloaderMessage(data);
    if (message === null) {
      return false;
    }

    if (message.command === COMMAND.WRITE) {
      this.handleBlockRequest(message.payload);
      return true;
    }
    if (message.command === COMMAND.FINISH) {
      this.handleFinish(message.payload);
      return true;
    }
    return false;
  }

  handleBlockRequest(payload) {
    if (this.state !== "waiting-for-request") {
      return this.fail("Received a block request after sending UPDATE_FINISH");
    }
    if (payload.length !== 3 || payload.some((value) => value > 0x7f)) {
      return this.fail("Bootloader sent an invalid block request");
    }

    const requestedBlock = payload[0] | (payload[1] << 7) | (payload[2] << 14);
    if (requestedBlock === 0) {
      if (this.nextBlock !== this.firmware.writeMessages.length + 1) {
        return this.fail(
          `Bootloader requested final block before all writes were sent (${this.nextBlock - 1}/${
            this.firmware.writeMessages.length
          })`,
        );
      }
      this.clearTimeout();
      this.send(this.firmware.finishMessage);
      this.state = "waiting-for-finish";
      this.onStateChange(this.state);
      this.armTimeout("Timed out waiting for the final bootloader status");
      return;
    }

    if (requestedBlock === this.nextBlock) {
      if (requestedBlock > this.firmware.writeMessages.length) {
        return this.fail(`Bootloader requested out-of-range block ${requestedBlock}`);
      }
      this.clearTimeout();
      this.send(this.firmware.writeMessages[requestedBlock - 1]);
      this.lastBlock = requestedBlock;
      this.nextBlock += 1;
      this.onProgress({
        sentBlocks: requestedBlock,
        totalBlocks: this.firmware.writeMessages.length,
        duplicate: false,
      });
      this.armTimeout(`Timed out after sending firmware block ${requestedBlock}`);
      return;
    }

    if (requestedBlock === this.lastBlock) {
      this.clearTimeout();
      this.send(this.firmware.writeMessages[requestedBlock - 1]);
      this.onProgress({
        sentBlocks: requestedBlock,
        totalBlocks: this.firmware.writeMessages.length,
        duplicate: true,
      });
      this.armTimeout(`Timed out after resending firmware block ${requestedBlock}`);
      return;
    }

    this.fail(
      `Unexpected block request ${requestedBlock}; expected ${this.nextBlock}` +
        (this.lastBlock === null ? "" : ` or retry ${this.lastBlock}`),
    );
  }

  handleFinish(payload) {
    if (this.state !== "waiting-for-finish") {
      return this.fail("Received final status before UPDATE_FINISH was sent");
    }
    if (!arraysEqual(payload, [0x7f, 0x7f, 0x7f])) {
      return this.fail(
        `Bootloader rejected the update with status ${Array.from(payload, (value) =>
          value.toString(16).padStart(2, "0"),
        ).join(" ")}`,
      );
    }

    this.clearTimeout();
    this.state = "succeeded";
    this.onStateChange(this.state);
  }

  cancel() {
    if (["succeeded", "failed", "cancelled"].includes(this.state)) {
      return;
    }
    this.clearTimeout();
    this.state = "cancelled";
    this.onStateChange(this.state);
  }

  fail(message) {
    this.clearTimeout();
    this.state = "failed";
    this.onStateChange(this.state, message);
  }

  armTimeout(message) {
    this.clearTimeout();
    this.timer = setTimeout(() => this.fail(message), this.timeoutMs);
    this.timer.unref?.();
  }

  clearTimeout() {
    if (this.timer !== null) {
      clearTimeout(this.timer);
      this.timer = null;
    }
  }
}
