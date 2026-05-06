# MFA-Forge उपयोगकर्ता मार्गदर्शिका

## परिचय
MFA-Forge Windows के लिए local-first MFA प्रबंधक है। इसका मुख्य उद्देश्य TOTP खातों को एन्क्रिप्टेड वॉल्ट में सुरक्षित रखना और GUI, मानव CLI, स्थानीय एजेंट सत्र तथा MCP सर्वर के बीच एक समान कार्यप्रवाह देना है। एप्लिकेशन इस तरह बनाया गया है कि सीक्रेट स्थानीय मशीन से बाहर न जाएं, संवेदनशील कार्रवाइयां स्पष्ट स्वीकृति मांगें, और ऑटोमेशन unlock तथा grant सीमाओं को चुपचाप पार न कर सके।

## शुरुआत
पहली बार चलाने पर आपको master password बनाना होता है। यही वॉल्ट की मुख्य कुंजी है। इसके बिना आप खाते नहीं जोड़ सकते, seeds आयात नहीं कर सकते, backup निर्यात नहीं कर सकते, password rotate नहीं कर सकते, और token भी उत्पन्न नहीं कर सकते।

Master password दर्ज करने के बाद भी MFA-Forge इस release line में उपयोग की जाने वाली अतिरिक्त Windows verification चलाता है। व्यवहार में, app तभी उपयोग योग्य होती है जब दोनों चरण सफल हों।

Unlock के बाद मुख्य विंडो तीन कार्यक्षेत्रों में बंटी रहती है:

- बाईं ओर workspace tree
- बीच में account list
- उसी layout पर चलने वाले contextual dialogs और actions

इस संरचना का उद्देश्य है कि आप पहले scope चुनें और फिर उसी scope में काम करें, बिना अलग screen पर गए।

## Workspaces
Workspaces खातों को समूहित करने की प्रणाली हैं। इन्हें project, client, environment, या team के आधार पर tokens अलग रखने के लिए उपयोग करें।

ये कैसे काम करते हैं:

- root workspace सबसे ऊपर का container होता है
- subdirectory किसी मौजूदा workspace के भीतर nested path होती है
- कोई account workspace path के भीतर रह सकता है या unassigned रह सकता है

ये क्यों महत्वपूर्ण हैं:

- active workspace account view को filter करता है
- नए खाते डिफॉल्ट रूप से चुने गए workspace को inherit करते हैं
- export, restore, और review flows अधिक स्पष्ट हो जाते हैं जब accounts सुव्यवस्थित grouped हों

यदि आपके पास personal या emergency accounts हैं, तो उन्हें unassigned रखना उपयोगी हो सकता है।

## खाते जोड़ना
MFA-Forge TOTP account लोड करने के चार मुख्य तरीके देता है:

1. Manual entry
2. `otpauth://` URI import
3. QR image import
4. Compatible file import

Manual entry तब सही है जब आप service, user, workspace, algorithm, digits, और period सीधे नियंत्रित करना चाहते हैं।

URI, QR, या file import तब बेहतर है जब कोई दूसरा सिस्टम पहले से standard TOTP format में seed दे चुका हो। ऐसे में MFA-Forge source को parse करता है, account fields निकालता है, और secret को encrypted vault में संग्रहीत करता है।

महत्वपूर्ण व्यवहार:

- secrets UI में masked रहते हैं
- import dialogs बंद होते समय sensitive text साफ कर देते हैं
- metadata बदलने के लिए secret बदलना जरूरी नहीं है
- secret edit वैकल्पिक है; field खाली छोड़ने पर मौजूदा encrypted secret सुरक्षित रहता है

## टोकन और इतिहास
Token window code पढ़ने की operational view है। जब आप इसे किसी account row से खोलते हैं, MFA-Forge unlocked vault से वर्तमान TOTP value पढ़ता है और active period की countdown दिखाता है।

Refresh करते समय क्या अपेक्षा रखें:

- यदि वही TOTP period अभी भी active है, तो refresh वही code लौटा सकता है
- यदि period बदल गया, तो visible code तुरंत update हो जाता है
- code copy करने पर केवल वर्तमान token copy होता है, secret नहीं

History का उद्देश्य अलग है। यह token पढ़ने के लिए नहीं, बल्कि state recovery के लिए है।

Restore dialog आपको यह करने देता है:

- restorable snapshots देखना
- हटाए गए accounts वापस लाना
- किसी previous visible version को active vault में restore करना

History का उपयोग तब करें जब कोई account गलती से हट गया हो, metadata गलत बदल गई हो, या आपको account फिर से manually बनाए बिना previous version पर लौटना हो।

## बैकअप और आयात
Export एक encrypted MFA-Forge backup बनाता है। इसका उद्देश्य पूरे vault को ऐसे format में सुरक्षित रखना है जिसे बाद में MFA-Forge फिर से import कर सके।

Import का प्रभाव जानबूझकर मजबूत रखा गया है: validation के बाद यह active vault की current contents को imported encrypted backup से replace कर देता है। यह disaster recovery या machine migration के लिए उपयोगी है, लेकिन इसे merge नहीं बल्कि controlled restore की तरह समझना चाहिए।

अनुशंसित अभ्यास:

- बड़े बदलाव या bulk import से पहले backup बनाएं
- backups को protected location में रखें
- apply करने से पहले सुनिश्चित करें कि आप वही backup import कर रहे हैं जिसकी आपको अपेक्षा है

## स्थानीय एजेंट और MCP
स्थानीय agent session और MCP server local automation के लिए हैं, लेकिन वे permanently trusted channels नहीं हैं।

मुख्य व्यवहार:

- दोनों `deny-by-default` स्थिति से शुरू होते हैं
- session खोलने के लिए native unlock flow आवश्यक है
- unlocked session केवल उतनी देर जीवित रहती है जितनी देर process चल रहा हो
- sensitive operations के लिए explicit grants या dedicated prompts चाहिए

सुरक्षित रखी गई कार्रवाइयों के उदाहरण:

- किसी account के लिए token generate करना
- accounts provision या import करना
- sensitive history या audit data पढ़ना
- master password rotate करना

इसका अर्थ है कि automation संभव है, लेकिन वह user approval और local session lifetime से बंधी रहती है।

## समस्या निवारण
यदि unlock विफल हो:

- पहले master password जांचें
- फिर Windows verification prompt को पूरा करें यदि वह दिखाई दे
- यदि app फिर से loader पर लौट आए, flow दोबारा चलाएं और main window के बाहर native prompt देखें

यदि import विफल हो:

- जांचें कि source में अभी भी valid `otpauth://` payload है
- पुष्टि करें कि Base32 secret पूरी है
- पुष्टि करें कि चुनी गई QR image वास्तव में अपेक्षित seed से संबंधित है

यदि token नहीं बदलता:

- current TOTP period के remaining seconds देखें
- period बदलने के बाद फिर से refresh करें

यदि automation denied हो:

- देखें कि session अभी भी open है या नहीं
- देखें कि required grant expire हो गई है या consume हो चुकी है
- जरूरत पड़ने पर local session फिर खोलें और वही exact action दोबारा approve करें
