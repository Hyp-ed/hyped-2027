// Copyright (C) 2023 The Qt Company Ltd.
// SPDX-License-Identifier: LicenseRef-Qt-Commercial OR BSD-3-Clause

import QtQuick
import QtQuick.Controls.Fusion
import QtQuick.Layouts

ApplicationWindow {
    id: window

    required property var backend

    readonly property string monospace: Qt.platform.os === "osx" ? "Menlo"
                                      : Qt.platform.os === "windows" ? "Consolas" : "monospace"
    readonly property color okColor: palette.window.hslLightness < 0.5 ? "#5fc77a" : "#1e8a3c"
    readonly property color pendingColor: palette.window.hslLightness < 0.5 ? "#e0b050" : "#a06c00"
    readonly property color failedColor: palette.window.hslLightness < 0.5 ? "#f07070" : "#c02828"
    readonly property int maxOutputLength: 200000

    function appendOutput(textArea, flickable, text) {
        const following = flickable.contentY + flickable.height >= flickable.contentHeight - 4
        textArea.insert(textArea.length, text)
        if (textArea.length > maxOutputLength)
            textArea.remove(0, textArea.length - maxOutputLength)
        if (following)
            Qt.callLater(() => flickable.contentY = Math.max(0, flickable.contentHeight - flickable.height))
    }

    width: 1280
    height: 800
    minimumWidth: 800
    minimumHeight: 500
    visible: true
    title: qsTr("HYPED Simulation")

    SplitView {
        anchors.fill: parent

        ColumnLayout {
            SplitView.fillWidth: true
            SplitView.minimumWidth: 400
            spacing: 0

            ToolBar {
                Layout.fillWidth: true

                RowLayout {
                    anchors.fill: parent

                    Label {
                        leftPadding: 8
                        text: qsTr("UART")
                        font.bold: true
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    ToolButton {
                        text: qsTr("Tile")
                        enabled: uartPanels.count > 0
                        onClicked: workspace.tile()
                    }
                    ToolButton {
                        text: qsTr("Add panel")
                        onClicked: workspace.addPanel()
                    }
                }
            }

            Item {
                id: workspace

                // While set, panels follow the grid. Moving or resizing one by hand clears it.
                property bool tiled: true
                property int nextPanelNumber: 1
                property int topZ: 1
                readonly property int gap: 8
                readonly property int snap: 8

                function addPanel() {
                    uartPanels.append({ name: qsTr("UART %1").arg(nextPanelNumber++) })
                    if (tiled)
                        tile()
                }

                function tile() {
                    tiled = true
                    const count = panelRepeater.count
                    if (count === 0)
                        return
                    const columns = Math.max(1, Math.min(count, Math.round(Math.sqrt(count * width / height))))
                    const rows = Math.ceil(count / columns)
                    const cellHeight = (height - gap) / rows
                    for (let i = 0; i < count; i++) {
                        const row = Math.floor(i / columns)
                        // The last row stretches when it isn't full
                        const rowColumns = row === rows - 1 ? count - row * columns : columns
                        const cellWidth = (width - gap) / rowColumns
                        const panel = panelRepeater.itemAt(i)
                        panel.x = gap + (i - row * columns) * cellWidth
                        panel.y = gap + row * cellHeight
                        panel.width = cellWidth - gap
                        panel.height = cellHeight - gap
                    }
                }

                function keepPanelsInside() {
                    if (tiled) {
                        tile()
                        return
                    }
                    for (let i = 0; i < panelRepeater.count; i++) {
                        const panel = panelRepeater.itemAt(i)
                        panel.x = Math.max(0, Math.min(panel.x, width - panel.width))
                        panel.y = Math.max(0, Math.min(panel.y, height - panel.height))
                    }
                }

                Layout.fillWidth: true
                Layout.fillHeight: true
                clip: true
                onWidthChanged: keepPanelsInside()
                onHeightChanged: keepPanelsInside()

                ListModel {
                    id: uartPanels
                }

                ColumnLayout {
                    anchors.centerIn: parent
                    width: Math.min(parent.width - 48, 460)
                    spacing: 12
                    visible: uartPanels.count === 0

                    Label {
                        Layout.fillWidth: true
                        text: qsTr("No UART panels")
                        font.bold: true
                        horizontalAlignment: Text.AlignHCenter
                    }
                    Label {
                        Layout.fillWidth: true
                        text: qsTr("Panels read from Renode server socket terminals. Open one for each UART in your script, then add a panel with the same port:")
                        wrapMode: Text.Wrap
                        horizontalAlignment: Text.AlignHCenter
                    }
                    TextEdit {
                        Layout.fillWidth: true
                        text: "emulation CreateServerSocketTerminal 3456 \"uart1\"\nconnector Connect sysbus.usart1 uart1"
                        readOnly: true
                        selectByMouse: true
                        wrapMode: TextEdit.WrapAnywhere
                        color: palette.windowText
                        selectionColor: palette.highlight
                        selectedTextColor: palette.highlightedText
                        font.family: window.monospace
                    }
                    Button {
                        Layout.alignment: Qt.AlignHCenter
                        text: qsTr("Add panel")
                        onClicked: workspace.addPanel()
                    }
                }

                Repeater {
                    id: panelRepeater

                    model: uartPanels

                    delegate: FocusScope {
                        id: panel

                        required property int index
                        required property string name

                        property int subscription: -1
                        property int port: 0
                        property bool connected: false
                        property bool hasOutput: false
                        property bool renaming: false

                        function raise() {
                            z = ++workspace.topZ
                        }

                        function setPort(newPort) {
                            if (newPort === port)
                                return
                            if (subscription >= 0)
                                window.backend.unsubscribe_uart(subscription)
                            port = newPort
                            connected = false
                            subscription = port > 0 ? window.backend.subscribe_uart(port) : -1
                        }

                        function arrangedByHand() {
                            workspace.tiled = false
                        }

                        function snapToGrid() {
                            x = Math.round(x / workspace.snap) * workspace.snap
                            y = Math.round(y / workspace.snap) * workspace.snap
                            workspace.keepPanelsInside()
                        }

                        Component.onCompleted: {
                            raise()
                            portField.forceActiveFocus()
                        }
                        Component.onDestruction: {
                            if (subscription >= 0)
                                window.backend.unsubscribe_uart(subscription)
                        }
                        onActiveFocusChanged: {
                            if (activeFocus)
                                raise()
                        }

                        Connections {
                            target: window.backend

                            function onUart_connected(id, connected) {
                                if (id === panel.subscription)
                                    panel.connected = connected
                            }

                            function onUart_text(id, text) {
                                if (id === panel.subscription) {
                                    panel.hasOutput = true
                                    window.appendOutput(output, outputView.contentItem, text)
                                }
                            }
                        }

                        Rectangle {
                            anchors.fill: parent
                            color: palette.base
                            border.color: panel.activeFocus ? palette.highlight : palette.mid
                        }

                        ColumnLayout {
                            anchors.fill: parent
                            anchors.margins: 1
                            spacing: 0

                            Rectangle {
                                Layout.fillWidth: true
                                Layout.preferredHeight: header.implicitHeight + 8
                                color: palette.window

                                MouseArea {
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
                                    ToolTip.visible: containsMouse && !pressed
                                    ToolTip.delay: 1000
                                    ToolTip.text: qsTr("Drag to move, double-click to rename")
                                    drag.target: panel
                                    drag.threshold: 2
                                    drag.minimumX: 0
                                    drag.minimumY: 0
                                    drag.maximumX: workspace.width - panel.width
                                    drag.maximumY: workspace.height - panel.height
                                    drag.onActiveChanged: {
                                        if (drag.active)
                                            panel.arrangedByHand()
                                        else
                                            panel.snapToGrid()
                                    }
                                    onPressed: panel.forceActiveFocus()
                                    onDoubleClicked: {
                                        panel.renaming = true
                                        nameField.selectAll()
                                        nameField.forceActiveFocus()
                                    }
                                }

                                RowLayout {
                                    id: header

                                    anchors.fill: parent
                                    anchors.margins: 4
                                    anchors.leftMargin: 8
                                    spacing: 6

                                    Rectangle {
                                        Layout.preferredWidth: 8
                                        Layout.preferredHeight: 8
                                        radius: 4
                                        color: panel.connected ? window.okColor
                                             : panel.port > 0 ? window.pendingColor : palette.mid
                                    }
                                    Label {
                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 40
                                        visible: !panel.renaming
                                        text: panel.name
                                        font.bold: true
                                        elide: Text.ElideRight
                                    }
                                    TextField {
                                        id: nameField

                                        Layout.fillWidth: true
                                        Layout.minimumWidth: 40
                                        visible: panel.renaming
                                        text: panel.name
                                        font.bold: true
                                        selectByMouse: true
                                        Accessible.name: qsTr("Panel name")
                                        onEditingFinished: {
                                            if (text !== "")
                                                uartPanels.setProperty(panel.index, "name", text)
                                            panel.renaming = false
                                        }
                                    }
                                    Label {
                                        text: panel.connected ? qsTr("Connected")
                                            : panel.port > 0 ? qsTr("Waiting") : qsTr("No port")
                                        opacity: 0.75
                                    }
                                    TextField {
                                        id: portField

                                        Layout.preferredWidth: 68
                                        placeholderText: qsTr("Port")
                                        selectByMouse: true
                                        horizontalAlignment: TextInput.AlignRight
                                        inputMethodHints: Qt.ImhDigitsOnly
                                        validator: IntValidator {
                                            bottom: 1
                                            top: 65535
                                        }
                                        Accessible.name: qsTr("UART port")
                                        onEditingFinished: panel.setPort(acceptableInput ? parseInt(text) : 0)
                                    }
                                    ToolButton {
                                        text: qsTr("Clear")
                                        enabled: panel.hasOutput
                                        onClicked: {
                                            output.clear()
                                            panel.hasOutput = false
                                        }
                                    }
                                    ToolButton {
                                        text: "×"
                                        Accessible.name: qsTr("Close panel")
                                        onClicked: uartPanels.remove(panel.index)
                                    }
                                }
                            }

                            Item {
                                Layout.fillWidth: true
                                Layout.fillHeight: true

                                ScrollView {
                                    id: outputView

                                    anchors.fill: parent

                                    TextArea {
                                        id: output

                                        readOnly: true
                                        selectByMouse: true
                                        textFormat: TextEdit.PlainText
                                        wrapMode: TextEdit.WrapAnywhere
                                        font.family: window.monospace
                                        background: null
                                    }
                                }

                                Label {
                                    anchors.centerIn: parent
                                    width: parent.width - 32
                                    visible: !panel.hasOutput
                                    wrapMode: Text.Wrap
                                    horizontalAlignment: Text.AlignHCenter
                                    opacity: 0.6
                                    text: panel.connected ? qsTr("Connected. Nothing received yet.")
                                        : panel.port > 0 ? qsTr("Waiting for Renode to open port %1").arg(panel.port)
                                        : qsTr("Enter the port of a Renode server socket terminal")
                                }
                            }
                        }

                        ResizeGrip {
                            target: panel
                            bounds: workspace
                            resizesWidth: true
                            anchors.right: parent.right
                            anchors.top: parent.top
                            anchors.bottom: parent.bottom
                            anchors.bottomMargin: 12
                            width: 5
                            cursorShape: Qt.SizeHorCursor
                            onResizeStarted: panel.arrangedByHand()
                        }
                        ResizeGrip {
                            target: panel
                            bounds: workspace
                            resizesHeight: true
                            anchors.left: parent.left
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            anchors.rightMargin: 12
                            height: 5
                            cursorShape: Qt.SizeVerCursor
                            onResizeStarted: panel.arrangedByHand()
                        }
                        ResizeGrip {
                            target: panel
                            bounds: workspace
                            resizesWidth: true
                            resizesHeight: true
                            anchors.right: parent.right
                            anchors.bottom: parent.bottom
                            width: 12
                            height: 12
                            cursorShape: Qt.SizeFDiagCursor
                            onResizeStarted: panel.arrangedByHand()
                        }
                    }
                }
            }
        }

        ColumnLayout {
            id: scriptPanel

            function scriptName(path) {
                return path.substring(path.lastIndexOf("/") + 1)
            }

            function localPath(url) {
                const path = decodeURIComponent(url.toString().replace(/^file:\/\//, ""))
                return path.replace(/^\/([A-Za-z]:)/, "$1")
            }

            function loadDropped(urls) {
                const skipped = []
                for (const url of urls) {
                    const path = localPath(url)
                    if (path.toLowerCase().endsWith(".resc"))
                        scripts.append({ path: path, job: window.backend.load_script(path), status: "loading" })
                    else
                        skipped.push(scriptName(path))
                }
                dropMessage.text = skipped.length > 0
                    ? qsTr("Skipped %1: not a .resc script").arg(skipped.join(", ")) : ""
            }

            function reload() {
                window.backend.clear_emulation()
                for (let i = 0; i < scripts.count; i++) {
                    scripts.setProperty(i, "status", "loading")
                    scripts.setProperty(i, "job", window.backend.load_script(scripts.get(i).path))
                }
            }

            SplitView.preferredWidth: 360
            SplitView.minimumWidth: 280
            spacing: 0

            Connections {
                target: window.backend

                function onMonitor_job_finished(id, succeeded) {
                    for (let i = 0; i < scripts.count; i++) {
                        if (scripts.get(i).job === id)
                            scripts.setProperty(i, "status", succeeded ? "loaded" : "failed")
                    }
                }

                function onMonitor_output(text) {
                    window.appendOutput(monitorOutput, monitorView.contentItem, text)
                }
            }

            ListModel {
                id: scripts
            }

            ToolBar {
                Layout.fillWidth: true

                RowLayout {
                    anchors.fill: parent

                    Label {
                        leftPadding: 8
                        text: qsTr("Renode scripts")
                        font.bold: true
                    }
                    Item {
                        Layout.fillWidth: true
                    }
                    ToolButton {
                        text: qsTr("Reload")
                        enabled: scripts.count > 0
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: qsTr("Clear the emulation and load these scripts again")
                        onClicked: scriptPanel.reload()
                    }
                    ToolButton {
                        text: qsTr("Clear")
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: qsTr("Remove every machine from the emulation")
                        onClicked: {
                            window.backend.clear_emulation()
                            scripts.clear()
                            dropMessage.text = ""
                        }
                    }
                }
            }

            Rectangle {
                Layout.fillWidth: true
                Layout.preferredHeight: 104
                Layout.margins: 8
                radius: 4
                color: dropArea.containsDrag ? Qt.alpha(palette.highlight, 0.2) : "transparent"
                border.color: dropArea.containsDrag ? palette.highlight : palette.mid
                border.width: dropArea.containsDrag ? 2 : 1

                DropArea {
                    id: dropArea

                    anchors.fill: parent
                    onDropped: drop => {
                        if (!drop.hasUrls)
                            return
                        scriptPanel.loadDropped(drop.urls)
                        drop.acceptProposedAction()
                    }
                }

                Label {
                    anchors.centerIn: parent
                    width: parent.width - 24
                    wrapMode: Text.Wrap
                    horizontalAlignment: Text.AlignHCenter
                    text: dropArea.containsDrag ? qsTr("Release to load") : qsTr("Drop .resc scripts here")
                }
            }

            Label {
                id: dropMessage

                Layout.fillWidth: true
                Layout.leftMargin: 8
                Layout.rightMargin: 8
                Layout.bottomMargin: 8
                visible: text !== ""
                wrapMode: Text.Wrap
                color: window.failedColor
            }

            ListView {
                Layout.fillWidth: true
                Layout.preferredHeight: Math.min(contentHeight, 160)
                Layout.bottomMargin: count > 0 ? 8 : 0
                clip: true
                model: scripts

                ScrollBar.vertical: ScrollBar {}

                delegate: ItemDelegate {
                    required property string path
                    required property string status

                    width: ListView.view.width
                    hoverEnabled: true
                    ToolTip.visible: hovered
                    ToolTip.text: path

                    contentItem: RowLayout {
                        Label {
                            Layout.fillWidth: true
                            text: scriptPanel.scriptName(parent.parent.path)
                            elide: Text.ElideMiddle
                        }
                        Label {
                            readonly property string status: parent.parent.status

                            text: status === "loaded" ? qsTr("Loaded")
                                : status === "failed" ? qsTr("Failed") : qsTr("Loading")
                            color: status === "loaded" ? window.okColor
                                 : status === "failed" ? window.failedColor : window.pendingColor
                        }
                    }
                }
            }

            Label {
                Layout.leftMargin: 8
                Layout.bottomMargin: 4
                text: qsTr("Monitor output")
                font.bold: true
            }

            ScrollView {
                id: monitorView

                Layout.fillWidth: true
                Layout.fillHeight: true

                TextArea {
                    id: monitorOutput

                    readOnly: true
                    selectByMouse: true
                    textFormat: TextEdit.PlainText
                    wrapMode: TextEdit.WrapAnywhere
                    font.family: window.monospace
                }
            }
        }
    }

    component ResizeGrip: MouseArea {
        required property Item target
        required property Item bounds
        property bool resizesWidth: false
        property bool resizesHeight: false
        readonly property int minimumWidth: 320
        readonly property int minimumHeight: 120
        property point pressPosition
        property size pressSize

        signal resizeStarted

        hoverEnabled: true
        onPressed: mouse => {
            pressPosition = mapToItem(bounds, mouse.x, mouse.y)
            pressSize = Qt.size(target.width, target.height)
            target.forceActiveFocus()
            resizeStarted()
        }
        onPositionChanged: mouse => {
            if (!pressed)
                return
            const position = mapToItem(bounds, mouse.x, mouse.y)
            if (resizesWidth) {
                const width = pressSize.width + position.x - pressPosition.x
                target.width = Math.max(minimumWidth, Math.min(width, bounds.width - target.x))
            }
            if (resizesHeight) {
                const height = pressSize.height + position.y - pressPosition.y
                target.height = Math.max(minimumHeight, Math.min(height, bounds.height - target.y))
            }
        }
    }
}
